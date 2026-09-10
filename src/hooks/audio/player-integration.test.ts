import { beforeEach, describe, expect, it, vi } from 'vitest'
import { usePlayerStore } from '@/stores/player-store'
import { useRepeatStore } from '@/stores/repeat-store'
import type { Track } from '@/types'
import { connectPlayerEngine } from './player-integration'
import { createFakeAudioEngine } from './fake-engine'

const track = (id: number): Track => ({ id, duration: 120, title: 'Track', performer: null,
  tg_user_id: 1, file_id: '', file_unique_id: '', source: 'saved_music',
  mime_type: 'audio/mpeg', file_size: 100, created_at: '' })
const a = track(1)
const b = track(2)
function queue(tracks: Track[], cursor = 0) {
  usePlayerStore.setState({
    queue: { tracks, cursor, source: null, sourceIndices: null, baseEntries: null },
    currentTrack: tracks[cursor] ?? null, isPlaying: tracks.length > 0,
  })
}
function setup() {
  const fake = createFakeAudioEngine()
  const activity = { notifyPlaying: vi.fn(), notifyActivityStopped: vi.fn(), notifyCompleted: vi.fn() }
  return { ...fake, activity, disconnect: connectPlayerEngine(fake.engine, activity) }
}
beforeEach(() => {
  usePlayerStore.setState({ listenAttemptEpoch: 0 })
  queue([])
  useRepeatStore.setState({ repeat: 'none' })
})

describe('player engine integration', () => {
  it('retains rapid A → B → A transitions and ignores late state and queue advancement', async () => {
    const h = setup()
    queue([a, b])
    usePlayerStore.getState().playNext()
    usePlayerStore.getState().playPrevious()
    await Promise.resolve()
    expect(h.driver.sessions.map(session => session.request.trackId)).toEqual([1, 2, 1])
    expect(new Set(h.driver.sessions.map(session => session.request.attemptId)).size).toBe(3)
    const final = h.engine.getSnapshot()
    h.driver.sessions[0]!.observer.state({ status: 'paused', currentTimeSeconds: 100 })
    h.driver.sessions[1]!.observer.failed({ code: 'unknown', message: 'Late', recoverable: true })
    h.driver.sessions[0]!.observer.ended()
    h.driver.sessions[1]!.observer.ended()
    expect(h.engine.getSnapshot()).toBe(final)
    expect(usePlayerStore.getState().queue.cursor).toBe(0)
    expect(usePlayerStore.getState().isPlaying).toBe(true)
    expect(h.activity.notifyCompleted).not.toHaveBeenCalled()
    h.disconnect()
  })

  it('keeps buffering as intent to play, counts only actual playing, and syncs external pause', async () => {
    queue([a, b])
    const h = setup()
    await Promise.resolve()
    expect(h.activity.notifyPlaying).toHaveBeenCalledTimes(1)
    h.driver.sessions[0]!.observer.state({ status: 'buffering' })
    expect(usePlayerStore.getState().isPlaying).toBe(true)
    expect(h.activity.notifyActivityStopped).toHaveBeenCalledTimes(1)
    h.driver.sessions[0]!.observer.state({ status: 'playing' })
    h.driver.sessions[0]!.observer.state({ status: 'paused' })
    expect(usePlayerStore.getState().isPlaying).toBe(false)
    expect(h.driver.sessions).toHaveLength(1)
    usePlayerStore.getState().setPlaying(true)
    expect(h.engine.getSnapshot().attemptId).toBe(h.driver.sessions[0]!.request.attemptId)
    h.disconnect()
  })

  it('restarts duplicate rows on skip and completion as fresh transport attempts', async () => {
    queue([a, a, a])
    const h = setup()
    await Promise.resolve()
    await h.engine.seek(45)
    usePlayerStore.getState().playNext()
    await Promise.resolve()
    expect(h.engine.getSnapshot().currentTimeSeconds).toBe(0)
    h.driver.sessions[1]!.observer.ended()
    await Promise.resolve()
    expect(usePlayerStore.getState().queue.cursor).toBe(2)
    expect(h.driver.sessions).toHaveLength(3)
    expect(h.activity.notifyCompleted).toHaveBeenCalledExactlyOnceWith(true)
    h.disconnect()
  })

  it('repeats one with a new attempt, and does not advance the queue twice', async () => {
    queue([a, b])
    useRepeatStore.setState({ repeat: 'one' })
    const h = setup()
    await Promise.resolve()
    h.driver.sessions[0]!.observer.ended()
    h.driver.sessions[0]!.observer.ended()
    await Promise.resolve()
    expect(h.driver.sessions).toHaveLength(2)
    expect(h.driver.sessions[0]!.request.attemptId).not.toBe(h.driver.sessions[1]!.request.attemptId)
    expect(usePlayerStore.getState().queue.cursor).toBe(0)
    expect(h.activity.notifyCompleted).toHaveBeenCalledExactlyOnceWith(true)
    h.disconnect()
  })

  it('stops at the queue end, then starts a fresh attempt on Play', async () => {
    queue([a])
    const h = setup()
    await Promise.resolve()
    h.driver.sessions[0]!.observer.ended()
    expect(usePlayerStore.getState().isPlaying).toBe(false)
    usePlayerStore.getState().setPlaying(true)
    await Promise.resolve()
    expect(h.driver.sessions).toHaveLength(2)
    expect(h.engine.getSnapshot().status).toBe('playing')
    h.disconnect()
  })

  it('preserves an explicit seek after completion when Play opens the next attempt', async () => {
    queue([a])
    const h = setup()
    await Promise.resolve()
    h.driver.sessions[0]!.observer.ended()
    await h.engine.seek(40)
    usePlayerStore.getState().setPlaying(true)
    await Promise.resolve()
    expect(h.driver.sessions).toHaveLength(2)
    expect(h.engine.getSnapshot().currentTimeSeconds).toBe(40)
    h.disconnect()
  })

  it('pauses intent on error without advancing, and unloads on clear', async () => {
    queue([a, b])
    const h = setup()
    await Promise.resolve()
    h.driver.sessions[0]!.observer.failed({ code: 'decode-failed', message: 'Failed', recoverable: true })
    expect(usePlayerStore.getState().isPlaying).toBe(false)
    expect(usePlayerStore.getState().queue.cursor).toBe(0)
    expect(h.activity.notifyCompleted).not.toHaveBeenCalled()
    queue([])
    expect(h.engine.getSnapshot().status).toBe('idle')
    expect(h.driver.sessions[0]!.disposed).toBe(true)
    h.disconnect()
  })
})
