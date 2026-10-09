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
