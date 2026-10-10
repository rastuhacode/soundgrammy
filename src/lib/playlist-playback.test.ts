import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { useLibraryStore } from '@/stores/library-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useSessionStore } from '@/stores/session-store'
import { deferred, playlists, track } from '@/test-support/fixtures'
import { reorderPlaylistAndQueue } from './playlist-playback'

vi.mock('@/lib/api', () => ({ api: { reorderPlaylistTracks: vi.fn(), nativePlayerCommand: vi.fn(), nativeAudioSnapshot: vi.fn() } }))

beforeEach(() => {
  vi.resetAllMocks()
  useSessionStore.getState().clearSession()
  useLibraryStore.getState().setLibrary([track(1), track(2)])
  usePlaylistsStore.getState().hydrate(playlists([1, 1, 2]))
  vi.mocked(api.nativePlayerCommand).mockResolvedValue(undefined)
})

describe('playlist and playback coordination', () => {
  it('persists duplicate membership order before aligning the queue', async () => {
    const saved = deferred<string>()
    vi.mocked(api.reorderPlaylistTracks).mockReturnValueOnce(saved.promise)
    const pending = reorderPlaylistAndQueue(20, [1, 2, 1], [1, 1, 2], { fromIndex: 1, toIndex: 2 })
    await Promise.resolve()
    expect(api.nativePlayerCommand).not.toHaveBeenCalled()
    saved.resolve('new')
    await pending
    expect(usePlaylistsStore.getState().data?.custom[0]?.trackIds).toEqual([1, 2, 1])
    expect(api.nativePlayerCommand).toHaveBeenCalledWith({ type: 'realign', playlistId: 20,
      tracks: [track(1), track(2), track(1)], moveIndices: [1, 2] })
  })
  it('never aligns the queue when persistence fails', async () => {
    vi.mocked(api.reorderPlaylistTracks).mockRejectedValueOnce(new Error('Database unavailable'))
    await expect(reorderPlaylistAndQueue(20, [1, 2, 1], [1, 1, 2], { fromIndex: 1, toIndex: 2 })).rejects.toThrow('Database unavailable')
    expect(api.nativePlayerCommand).not.toHaveBeenCalled()
    expect(usePlaylistsStore.getState().data?.custom[0]?.trackIds).toEqual([1, 1, 2])
  })
})
