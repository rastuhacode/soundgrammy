import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { usePlaylistJobsStore } from './playlist-jobs-store'
import { useCacheStore } from './cache-store'
import { useSessionStore } from './session-store'
import { deferred } from '@/test-support/fixtures'
import type { PlaylistDownloadResult } from '@/types'
vi.mock('@/lib/api', () => ({ api: { downloadPlaylist: vi.fn(), cacheTracks: vi.fn() } }))
vi.mock('@tauri-apps/plugin-opener', () => ({ revealItemInDir: vi.fn() }))
beforeEach(() => {
  vi.resetAllMocks()
  useSessionStore.getState().clearSession()
  usePlaylistJobsStore.getState().reset()
  useCacheStore.getState().clearAll()
})
describe('playlist job account ownership', () => {
  it('keeps busy ownership until overlapping cache and download jobs both finish', async () => {
    const download = deferred<PlaylistDownloadResult>()
    const cache = deferred<number[]>()
    vi.mocked(api.downloadPlaylist).mockReturnValueOnce(download.promise)
    vi.mocked(api.cacheTracks).mockReturnValueOnce(cache.promise)
    const input = { playlistId: 20, name: 'Shared', trackIds: [1] }
    const first = usePlaylistJobsStore.getState().runDownloadPlaylist(input)
    const second = usePlaylistJobsStore.getState().runCachePlaylist(input)
    expect(useCacheStore.getState().busyCounts.get(1)).toBe(2)
    download.resolve({ folderPath: null, succeeded: [], failed: [] })
    await first
    expect(useCacheStore.getState().isBusy(1)).toBe(true)
    expect(usePlaylistJobsStore.getState().isCaching(20)).toBe(true)
    cache.resolve([1])
    await second
    expect(useCacheStore.getState().isBusy(1)).toBe(false)
    expect(useCacheStore.getState().isCached(1)).toBe(true)
    expect(usePlaylistJobsStore.getState().jobsById).toEqual({})
  })
  it('drops late results without clearing a new account busy track', async () => {
    const pending = deferred<PlaylistDownloadResult>()
    vi.mocked(api.downloadPlaylist).mockReturnValue(pending.promise)
    const job = usePlaylistJobsStore.getState().runDownloadPlaylist({ playlistId: 20, name: 'Old', trackIds: [1] })
    useSessionStore.getState().clearSession()
    usePlaylistJobsStore.getState().reset()
    useCacheStore.getState().clearAll()
    useCacheStore.getState().markBusy([1])
    pending.resolve({ folderPath: null, succeeded: [], failed: [] })
    await job
    expect(usePlaylistJobsStore.getState().resultQueue).toEqual([])
    expect(useCacheStore.getState().isBusy(1)).toBe(true)
  })
})
