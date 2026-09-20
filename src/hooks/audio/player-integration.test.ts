import { beforeEach, describe, expect, it, vi } from 'vitest'
import { acceptPlayerResponse, usePlayerStore } from '@/stores/player-store'
import type { AudioEngine, AudioEngineEvent, AudioEngineSnapshot } from './engine'
import { connectPlayerEngine } from './player-integration'
import type { Track } from '@/types'

const track = (id: number): Track => ({ id, duration: 120, title: 'Track', performer: null, tg_user_id: 1, file_id: '', file_unique_id: '', source: 'saved_music', mime_type: 'audio/mpeg', file_size: 100, created_at: '' })
function snapshot(attempt = 1, cursor = 0, status: AudioEngineSnapshot['status'] = 'playing'): AudioEngineSnapshot {
  return { revision: attempt, kind: 'native-rust', status, trackId: 1, attemptId: `native:${attempt}`, currentTimeSeconds: 12, durationSeconds: 120,
    volumePercent: 100, error: null, initialLoading: false, bufferedRanges: [],
    player: { revision: attempt, attempt, endReason: 'completed', isPlaying: status === 'playing', preferences: { repeat: 'none', shuffle: 'off', mode: 'random' },
      queue: { tracks: [track(1), track(1)], cursor, source: null, sourceIndices: null, baseEntries: null } } }
}
function harness(initial = snapshot()) {
  let state = initial
  const listeners = new Set<(event: AudioEngineEvent) => void>()
  const engine = {
    kind: 'native-rust',
    getSnapshot: () => state,
    subscribe: (fn: (e: AudioEngineEvent) => void) => {
      listeners.add(fn)
      return () => {
        listeners.delete(fn)
      }
    },
    load: vi.fn(), unload: vi.fn(), play: vi.fn(), pause: vi.fn(), destroy: vi.fn() } as unknown as AudioEngine
  const activity = { notifyPlaying: vi.fn(), notifyActivityStopped: vi.fn(), notifyCompleted: vi.fn() }
  return { engine, activity,
    state: (next: AudioEngineSnapshot) => {
      state = next
      listeners.forEach(fn => fn({ type: 'state', snapshot: next }))
    },
    end: (attemptId: string) => listeners.forEach(fn => fn({ type: 'ended', trackId: state.trackId!, attemptId, revision: state.revision })),
  }
}
beforeEach(() => usePlayerStore.setState({ nativeRevision: -1 }))
describe('native player integration', () => {
  it('reattaches to the existing native session without loading or seeking', () => {
    const h = harness(snapshot(7, 1))
    const disconnect = connectPlayerEngine(h.engine, h.activity)
    expect(usePlayerStore.getState().queue.cursor).toBe(1)
    expect(usePlayerStore.getState().listenAttemptEpoch).toBe(7)
    expect(h.activity.notifyPlaying).toHaveBeenCalledTimes(1)
    expect(h.engine.load).not.toHaveBeenCalled()
    disconnect()
    expect(h.engine.unload).not.toHaveBeenCalled()
    expect(h.engine.pause).not.toHaveBeenCalled()
  })
  it('never advances on ended; only native snapshots move the queue', () => {
    const h = harness()
    const disconnect = connectPlayerEngine(h.engine, h.activity)
    h.end('native:1')
    h.end('native:1')
    h.end('old')
    expect(h.activity.notifyCompleted).toHaveBeenCalledExactlyOnceWith(false)
    expect(usePlayerStore.getState().queue.cursor).toBe(0)
    h.state(snapshot(2, 1))
    expect(usePlayerStore.getState().queue.cursor).toBe(1)
    expect(h.engine.load).not.toHaveBeenCalled()
    disconnect()
  })
  it('tracks actual activity without writing pause/play commands back to Rust', () => {
    const h = harness()
    const disconnect = connectPlayerEngine(h.engine, h.activity)
    h.state({ ...snapshot(), status: 'buffering' })
    h.state({ ...snapshot(), status: 'playing' })
    h.state(snapshot(2, 0, 'paused'))
    expect(h.activity.notifyActivityStopped).toHaveBeenCalledTimes(1)
    expect(h.activity.notifyPlaying).toHaveBeenCalledTimes(2)
    expect(h.engine.play).not.toHaveBeenCalled()
    expect(h.engine.pause).not.toHaveBeenCalled()
    disconnect()
  })
  it.each([1, 2])('ignores delayed completion after the store advances to track %s', (nextTrackId) => {
    const h = harness()
    const disconnect = connectPlayerEngine(h.engine, h.activity)
    const next = snapshot(2, 1)
    next.trackId = nextTrackId
    next.player!.queue.tracks[1] = track(nextTrackId)
    // Command replies can advance the mirror before old transport events arrive.
    acceptPlayerResponse(next)
    h.state({ ...snapshot(), status: 'ended' })
    h.end('native:1')
    expect(h.activity.notifyCompleted).not.toHaveBeenCalled()
    expect(usePlayerStore.getState().listenAttemptEpoch).toBe(2)
    h.state(next)
    h.end('native:2')
    expect(h.activity.notifyCompleted).toHaveBeenCalledExactlyOnceWith(false)
    disconnect()
  })

  it('does not mistake a newer merged session for the old transport attempt', () => {
    const h = harness()
    const disconnect = connectPlayerEngine(h.engine, h.activity)
    const next = snapshot(2, 1)
    acceptPlayerResponse(next)
    // Queue and transport revisions reconcile independently. A late full queue
    // snapshot can temporarily coexist with the old duplicate's audio identity.
    h.state({ ...snapshot(), player: next.player, status: 'ended' })
    h.end('native:1')
    expect(h.activity.notifyCompleted).not.toHaveBeenCalled()
    expect(h.activity.notifyActivityStopped).not.toHaveBeenCalled()
    h.state(next)
    expect(h.activity.notifyPlaying).toHaveBeenCalledTimes(2)
    disconnect()
  })
})
