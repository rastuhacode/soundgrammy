import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { useCacheStore } from './cache-store'
import { useListenStatsStore } from './listen-stats-store'
import { useSessionStore } from './session-store'
import { deferred } from '@/test-support/fixtures'
import type { TrackListenStats } from '@/types'

vi.mock('@/lib/api', () => ({ api: {
  getCacheStatus: vi.fn(), listListenStats: vi.fn(),
  getListenStatisticsEnabled: vi.fn(async () => true),
} }))
const stats = (trackId: number, starts = 1): TrackListenStats => ({
  track_id: trackId, starts, qualified_plays: 0, completes: 0, early_skips: 0,
  total_listened_ms: 0, first_played_at_ms: null, last_played_at_ms: null, likeness: 0,
})
beforeEach(() => {
  vi.clearAllMocks()
  useSessionStore.getState().clearSession()
  useCacheStore.getState().clearAll()
  useListenStatsStore.getState().reset()
})

describe('cache snapshot reconciliation', () => {
  it('retains untouched snapshot tracks and replays additions and removals', async () => {
    const pending = deferred<number[]>()
    vi.mocked(api.getCacheStatus).mockReturnValueOnce(pending.promise)
    const request = useCacheStore.getState().hydrate()
    useCacheStore.getState().markCached([3])
    // Track 1 has not reached the mirror yet, but its removal must win.
    useCacheStore.getState().markUncached([1])
    pending.resolve([1, 2])
    await request
    expect([...useCacheStore.getState().cachedIds].sort()).toEqual([2, 3])
    expect(useCacheStore.getState().hydrated).toBe(true)
  })

  it.each(['clear', 'logout'] as const)('does not restore a snapshot after %s', async (action) => {
    const pending = deferred<number[]>()
    vi.mocked(api.getCacheStatus).mockReturnValueOnce(pending.promise)
    const request = useCacheStore.getState().hydrate()
    if (action === 'logout') useSessionStore.getState().clearSession()
    useCacheStore.getState().clearAll()
    useCacheStore.getState().markCached([3])
    pending.resolve([1, 2])
    await request
    expect([...useCacheStore.getState().cachedIds]).toEqual([3])
  })

  it('ignores an older request finishing after a newer snapshot', async () => {
    const older = deferred<number[]>()
    vi.mocked(api.getCacheStatus).mockReturnValueOnce(older.promise).mockResolvedValueOnce([2])
    const request = useCacheStore.getState().hydrate()
    await useCacheStore.getState().hydrate()
    older.resolve([1])
    await request
    expect([...useCacheStore.getState().cachedIds]).toEqual([2])
  })
})

describe('statistics snapshot reconciliation', () => {
  it('preserves live values while loading unrelated history', async () => {
    const pending = deferred<TrackListenStats[]>()
    vi.mocked(api.listListenStats).mockReturnValueOnce(pending.promise)
    const request = useListenStatsStore.getState().refresh()
    useListenStatsStore.getState().upsert(stats(1, 3))
    useListenStatsStore.getState().upsert(stats(3))
    pending.resolve([stats(1), stats(2)])
    await request
    const result = useListenStatsStore.getState().statsByTrackId
    expect([...result.keys()].sort()).toEqual([1, 2, 3])
    expect(result.get(1)?.starts).toBe(3)
  })

  it('keeps a clear and subsequent live updates ahead of an old snapshot', async () => {
    const pending = deferred<TrackListenStats[]>()
    vi.mocked(api.listListenStats).mockReturnValueOnce(pending.promise)
    const request = useListenStatsStore.getState().refresh()
    useListenStatsStore.getState().clear()
    useListenStatsStore.getState().upsert(stats(3))
    pending.resolve([stats(1), stats(2)])
    await request
    expect([...useListenStatsStore.getState().statsByTrackId.keys()]).toEqual([3])
  })

  it('ignores an older request finishing after a newer snapshot', async () => {
    const older = deferred<TrackListenStats[]>()
    vi.mocked(api.listListenStats).mockReturnValueOnce(older.promise).mockResolvedValueOnce([stats(2)])
    const request = useListenStatsStore.getState().refresh()
    await useListenStatsStore.getState().refresh()
    older.resolve([stats(1)])
    await request
    expect([...useListenStatsStore.getState().statsByTrackId.keys()]).toEqual([2])
  })
})
