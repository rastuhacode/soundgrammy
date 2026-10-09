import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import type { Track } from '@/types'
import type { PlaybackSession } from '@/types/playback'
import { acceptPlayerResponse, applyPlaybackSession, installPlayerCommands, sendPlayerCommand, usePlayerStore } from './player-store'
import { useSessionStore } from './session-store'
import { deferred } from '@/test-support/fixtures'
import { useRepeatStore } from './repeat-store'
import { useShuffleStore } from './shuffle-store'

vi.mock('@/lib/api', () => ({ api: { nativePlayerCommand: vi.fn(), nativeAudioSnapshot: vi.fn() } }))
const track = (id: number): Track => ({ id, tg_user_id: 1, file_id: '', file_unique_id: '', title: 'Track', performer: null,
  duration: 120, source: 'saved_music', mime_type: 'audio/mpeg', file_size: 100, created_at: '' })
function session(revision = 1, cursor = 0, attempt = 1): PlaybackSession {
  return { revision, attempt, isPlaying: true, endReason: 'replaced', preferences: { repeat: 'none', shuffle: 'off', mode: 'random' },
    queue: { tracks: [track(1), track(1), track(2)], cursor, source: null, sourceIndices: null, baseEntries: null } }
}
beforeEach(() => {
  vi.clearAllMocks()
  useSessionStore.getState().clearSession()
  vi.mocked(api.nativePlayerCommand).mockResolvedValue(undefined)
  vi.mocked(api.nativeAudioSnapshot).mockResolvedValue(undefined)
  usePlayerStore.setState({ nativeRevision: -1, commandError: null, currentTrack: null, isPlaying: false, listenAttemptEpoch: 0,
    queue: { tracks: [], cursor: -1, source: null, sourceIndices: null, baseEntries: null } })
})

describe('native player mirror', () => {
  it('clears all presentation state immediately when its account lifetime ends', () => {
    applyPlaybackSession(session(5, 2))
    usePlayerStore.setState({ commandError: 'Old failure' })
    useSessionStore.getState().clearSession()
    usePlayerStore.getState().reset()
    expect(usePlayerStore.getState()).toMatchObject({ currentTrack: null, isPlaying: false,
      nativeRevision: -1, commandError: null, queue: { tracks: [], cursor: -1 } })
    expect(api.nativePlayerCommand).not.toHaveBeenCalled()
  })
  it('does not block new-account commands behind an old request or accept its response', async () => {
    const old = deferred<unknown>()
    vi.mocked(api.nativePlayerCommand).mockReturnValueOnce(old.promise).mockResolvedValueOnce({ player: session(2) })
    const first = sendPlayerCommand({ type: 'next' }).catch(() => {})
    await vi.waitFor(() => expect(api.nativePlayerCommand).toHaveBeenCalledTimes(1))
    useSessionStore.getState().clearSession()
    await sendPlayerCommand({ type: 'playTrack', track: track(2) })
    old.resolve({ player: session(100, 2) })
    await first
    expect(usePlayerStore.getState().nativeRevision).toBe(2)
  })
  it('sends player actions to the injected command port without native IPC', async () => {
    const command = vi.fn(async () => ({ player: session(1) }))
    const restore = installPlayerCommands({ command, snapshot: async () => undefined })
    try {
      usePlayerStore.getState().playNext()
      await sendPlayerCommand({ type: 'toggle' })
      expect(command.mock.calls).toEqual([[{ type: 'next' }], [{ type: 'toggle' }]])
      expect(api.nativePlayerCommand).not.toHaveBeenCalled()
      expect(usePlayerStore.getState().currentTrack?.id).toBe(1)
    }
    finally { restore() }
  })
  it('never changes queue or playing state before the native acknowledgement', async () => {
    let finish!: (value: unknown) => void
    vi.mocked(api.nativePlayerCommand).mockReturnValueOnce(new Promise((resolve) => {
      finish = resolve
    }))
    const pending = sendPlayerCommand({ type: 'playTrack', track: track(1) })
    await vi.waitFor(() => expect(finish).toBeTypeOf('function'))
    expect(usePlayerStore.getState().currentTrack).toBeNull()
    finish({ player: session() })
    await pending
    expect(usePlayerStore.getState().currentTrack?.id).toBe(1)
    expect(usePlayerStore.getState().isPlaying).toBe(true)
  })
  it('adopts native duplicate attempts, playback modes', () => {
    applyPlaybackSession(session())
    const next = { ...session(2, 1, 2), endReason: 'completed', preferences: { repeat: 'all' as const, shuffle: 'on' as const, mode: 'smart' as const } }
    applyPlaybackSession(next)
    expect(usePlayerStore.getState().queue.cursor).toBe(1)
    expect(usePlayerStore.getState().listenAttemptEpoch).toBe(2)
    expect(useRepeatStore.getState().repeat).toBe('all')
    expect(useShuffleStore.getState().mode).toBe('smart')
  })
  it('rejects stale snapshots and malformed IPC payloads', () => {
    acceptPlayerResponse({ player: session(5, 2, 5) })
    acceptPlayerResponse({ player: session(4, 0, 1) })
    acceptPlayerResponse({ player: { ...session(6), queue: { tracks: [], cursor: 40 } } })
    expect(usePlayerStore.getState().nativeRevision).toBe(5)
    expect(usePlayerStore.getState().currentTrack?.id).toBe(2)
  })
  it('serializes rapid next/previous requests without sending a stale replacement queue', async () => {
    applyPlaybackSession(session())
    usePlayerStore.getState().playNext()
    usePlayerStore.getState().playPrevious()
    await sendPlayerCommand({ type: 'toggle' })
    expect(vi.mocked(api.nativePlayerCommand).mock.calls.map(([c]) => c)).toEqual([
      { type: 'next' }, { type: 'previous', restart: false }, { type: 'toggle' },
    ])
    expect(usePlayerStore.getState().queue.cursor).toBe(0)
  })
  it('captures the viewed revision for index edits and never retries a stale edit', async () => {
    applyPlaybackSession(session(12))
    vi.mocked(api.nativePlayerCommand).mockRejectedValueOnce(new Error('Queue changed'))
    vi.mocked(api.nativeAudioSnapshot).mockResolvedValueOnce({ player: session(13, 1, 2) })
    await expect(sendPlayerCommand({ type: 'remove', indices: [0], queueRevision: usePlayerStore.getState().nativeRevision })).rejects.toThrow('Queue changed')
    expect(api.nativePlayerCommand).toHaveBeenCalledExactlyOnceWith({ type: 'remove', indices: [0], queueRevision: 12 })
    expect(usePlayerStore.getState().queue.cursor).toBe(1)
    expect(usePlayerStore.getState().commandError).toBe('Queue changed')
    await sendPlayerCommand({ type: 'playing', playing: false })
    expect(usePlayerStore.getState().commandError).toBeNull()
  })
  it('keeps sorted duplicate membership order when preparing a playlist command', async () => {
    const tracks = [track(1), track(1), track(2)]
    const orderedEntries = [{ track: tracks[2]!, sourceIndex: 2 }, { track: tracks[1]!, sourceIndex: 1 }, { track: tracks[0]!, sourceIndex: 0 }]
    usePlayerStore.getState().playPlaylist({ id: 1, name: 'List', tracks, trackIds: [1, 1, 2], isCustom: true }, { orderedEntries, startIndex: 1, shuffle: 'on' })
    await sendPlayerCommand({ type: 'attach' })
    expect(vi.mocked(api.nativePlayerCommand).mock.calls[0]?.[0]).toMatchObject({ type: 'playPlaylist', shuffle: 'on', pinStart: true, queue: { cursor: 1, sourceIndices: [2, 1, 0], baseEntries: orderedEntries } })
  })
  it('does not pin a starting row for toolbar shuffle', async () => {
    const tracks = [track(1), track(2)]
    usePlayerStore.getState().playPlaylist({ id: 1, name: 'List', tracks, trackIds: [1, 2], isCustom: true }, { shuffle: 'on' })
    await sendPlayerCommand({ type: 'attach' })
    expect(vi.mocked(api.nativePlayerCommand).mock.calls[0]?.[0]).toMatchObject({ type: 'playPlaylist', shuffle: 'on', pinStart: false })
  })
})
