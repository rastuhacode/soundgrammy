import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { hydrateLibrary } from './library-hydration'
import { useLibraryStore } from '@/stores/library-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { useSessionStore } from '@/stores/session-store'
import { deferred, playlists, track } from '@/test-support/fixtures'
import type { Track, TrackListenStats } from '@/types'

vi.mock('@/lib/api', () => ({ api: {
  listTracks: vi.fn(), listPlaylists: vi.fn(), getListenStatisticsEnabled: vi.fn(), listListenStats: vi.fn(),
  getCacheStatus: vi.fn(), nativePlayerCommand: vi.fn(), nativeAudioSnapshot: vi.fn(),
} }))
beforeEach(() => {
  vi.resetAllMocks()
  useSessionStore.getState().clearSession()
  useLibraryStore.getState().setLibrary([])
  usePlaylistsStore.getState().reset()
  useListenStatsStore.getState().reset()
  vi.mocked(api.listTracks).mockResolvedValue([track(1)])
  vi.mocked(api.listPlaylists).mockResolvedValue(playlists())
  vi.mocked(api.getCacheStatus).mockResolvedValue([])
  vi.mocked(api.getListenStatisticsEnabled).mockResolvedValue(true)
  vi.mocked(api.listListenStats).mockResolvedValue([])
})
const stats = (id = 1): TrackListenStats => ({ track_id: id, starts: 1, qualified_plays: 1, completes: 1,
  early_skips: 0, total_listened_ms: 120000, first_played_at_ms: 1, last_played_at_ms: 2, likeness: 1 })
describe('independent library hydration', () => {
  it('loads tracks and playlists when both optional statistics reads fail', async () => {
    vi.mocked(api.getListenStatisticsEnabled).mockRejectedValue('unavailable')
    vi.mocked(api.listListenStats).mockRejectedValue('unavailable')
    await hydrateLibrary(true)
    expect(useLibraryStore.getState().library.map(item => item.id)).toEqual([1])
    expect(usePlaylistsStore.getState().data?.custom[0]?.id).toBe(20)
  })
  it('does not restore old library data after logout', async () => {
    const pending = deferred<Track[]>()
    vi.mocked(api.listTracks).mockReturnValue(pending.promise)
    const request = useLibraryStore.getState().refresh()
    useSessionStore.getState().clearSession()
    useLibraryStore.getState().setLibrary([])
    pending.resolve([track(1)])
    await request
    expect(useLibraryStore.getState().library).toEqual([])
    expect(api.nativePlayerCommand).not.toHaveBeenCalled()
  })
  it('a newer library response wins when reads finish out of order', async () => {
    const first = deferred<Track[]>()
    vi.mocked(api.listTracks).mockReturnValueOnce(first.promise).mockResolvedValueOnce([track(2)])
    const request = useLibraryStore.getState().refresh()
    await useLibraryStore.getState().refresh()
    first.resolve([track(1)])
    await request
    expect(useLibraryStore.getState().library[0]?.id).toBe(2)
  })
  it('does not resurrect statistics cleared while a snapshot is in flight', async () => {
    const pending = deferred<TrackListenStats[]>()
    vi.mocked(api.listListenStats).mockReturnValue(pending.promise)
    const request = useListenStatsStore.getState().refresh()
    useListenStatsStore.getState().clear()
    pending.resolve([stats()])
    await request
    expect(useListenStatsStore.getState().statsByTrackId.size).toBe(0)
  })
  it('preserves a live listen upsert ahead of a stale snapshot', async () => {
    const pending = deferred<TrackListenStats[]>()
    vi.mocked(api.listListenStats).mockReturnValue(pending.promise)
    const request = useListenStatsStore.getState().refresh()
    useListenStatsStore.getState().upsert({ ...stats(), starts: 2 })
    pending.resolve([stats()])
    await request
    expect(useListenStatsStore.getState().statsByTrackId.get(1)?.starts).toBe(2)
  })
})
