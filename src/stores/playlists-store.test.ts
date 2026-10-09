import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { usePlaylistsStore } from './playlists-store'
import { useSessionStore } from './session-store'
import { deferred, playlists } from '@/test-support/fixtures'
import type { CustomPlaylistSummary, PlaylistsBundle } from '@/types'

vi.mock('@/lib/api', () => ({ api: {
  listPlaylists: vi.fn(), toggleLike: vi.fn(), createPlaylist: vi.fn(), updatePlaylist: vi.fn(),
  deletePlaylist: vi.fn(), addTracksToPlaylist: vi.fn(), removeTracksFromPlaylist: vi.fn(),
} }))
beforeEach(() => {
  vi.resetAllMocks()
  useSessionStore.getState().clearSession()
  usePlaylistsStore.getState().hydrate(playlists())
})
describe('playlist mutation ownership', () => {
  it('serializes reads and writes without losing a concurrent custom playlist', async () => {
    const like = deferred<PlaylistsBundle['liked']>()
    vi.mocked(api.toggleLike).mockReturnValue(like.promise)
    vi.mocked(api.createPlaylist).mockResolvedValue({ id: 30, name: 'New', trackIds: [], updatedAt: '' })
    const first = usePlaylistsStore.getState().toggleLike(4)
    const second = usePlaylistsStore.getState().createPlaylist('New')
    await Promise.resolve()
    expect(api.createPlaylist).not.toHaveBeenCalled()
    like.resolve({ id: 10, trackIds: [1, 2, 3, 4], updatedAt: 'new' })
    await Promise.all([first, second])
    expect(usePlaylistsStore.getState().data?.liked.trackIds).toEqual([1, 2, 3, 4])
    expect(usePlaylistsStore.getState().data?.custom.map(item => item.id)).toEqual([20, 30])
  })
  it('does not commit old-account responses or dispatch their queued writes', async () => {
    const create = deferred<CustomPlaylistSummary>()
    vi.mocked(api.createPlaylist).mockReturnValue(create.promise)
    const first = usePlaylistsStore.getState().createPlaylist('Old')
    const second = usePlaylistsStore.getState().deletePlaylist(20)
    const results = Promise.allSettled([first, second])
    await Promise.resolve()
    useSessionStore.getState().clearSession()
    usePlaylistsStore.getState().reset()
    create.resolve({ id: 30, name: 'Old', trackIds: [], updatedAt: '' })
    expect((await results).every(result => result.status === 'rejected')).toBe(true)
    expect(usePlaylistsStore.getState().data).toBeNull()
    expect(api.deletePlaylist).not.toHaveBeenCalled()
  })
  it('a new account does not wait on an old unresolved operation', async () => {
    const old = deferred<CustomPlaylistSummary>()
    vi.mocked(api.createPlaylist).mockReturnValueOnce(old.promise).mockResolvedValueOnce({ id: 31, name: 'New', trackIds: [], updatedAt: '' })
    const pending = usePlaylistsStore.getState().createPlaylist('Old').catch(() => {})
    await Promise.resolve()
    useSessionStore.getState().clearSession()
    usePlaylistsStore.getState().hydrate(playlists())
    await usePlaylistsStore.getState().createPlaylist('New')
    expect(usePlaylistsStore.getState().data?.custom.at(-1)?.id).toBe(31)
    old.resolve({ id: 30, name: 'Old', trackIds: [], updatedAt: '' })
    await pending
    expect(usePlaylistsStore.getState().data?.custom.some(item => item.id === 30)).toBe(false)
  })
  it('uses desired liked state, deduplicates inputs, and reports failures for retry', async () => {
    vi.mocked(api.toggleLike).mockRejectedValueOnce('offline').mockResolvedValueOnce({ id: 10, trackIds: [1, 2, 3, 5], updatedAt: '' })
    const result = await usePlaylistsStore.getState().setLiked([1, 4, 4, 5], true)
    expect(api.toggleLike).toHaveBeenCalledTimes(2)
    expect(result.failedTrackIds).toEqual([4])
    expect(usePlaylistsStore.getState().data?.liked.trackIds).toEqual([1, 2, 3, 5])
  })
  it('rejects stale positional edits before sending them to the backend', async () => {
    usePlaylistsStore.getState().setData(playlists([2, 3]))
    await expect(usePlaylistsStore.getState().removeTracks(20, [1], [1, 2, 3])).rejects.toThrow('playlist changed')
    expect(api.removeTracksFromPlaylist).not.toHaveBeenCalled()
  })
  it('preserves local membership when atomic bulk removal fails', async () => {
    vi.mocked(api.removeTracksFromPlaylist).mockRejectedValue('failed')
    await expect(usePlaylistsStore.getState().removeTracks(20, [0, 2], [1, 2, 3])).rejects.toBe('failed')
    expect(usePlaylistsStore.getState().data?.custom[0]?.trackIds).toEqual([1, 2, 3])
  })
  it('saves duplicate queue memberships in one create command', async () => {
    vi.mocked(api.createPlaylist).mockResolvedValue({ id: 30, name: 'Queue', trackIds: [1, 1, 2], updatedAt: '' })
    await usePlaylistsStore.getState().createPlaylist('Queue', [1, 1, 2])
    expect(api.createPlaylist).toHaveBeenCalledExactlyOnceWith({ name: 'Queue', trackIds: [1, 1, 2] })
    expect(api.addTracksToPlaylist).not.toHaveBeenCalled()
  })
})
