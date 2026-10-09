import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { deferred } from '@/test-support/fixtures'
import type { LastFmStatus } from '@/types'
import { useSessionStore } from './session-store'
import { useListenStatsStore } from './listen-stats-store'
import { useLastFmStore } from './lastfm-store'
import { useCacheStore } from './cache-store'

vi.mock('@/lib/api', () => ({ api: {
  setListenStatisticsEnabled: vi.fn(), getListenStatisticsEnabled: vi.fn(), clearListenStatistics: vi.fn(),
  setLastFmEnabled: vi.fn(), getLastFmStatus: vi.fn(),
  clearAudioCache: vi.fn(), getCacheUsage: vi.fn(),
} }))
const status: LastFmStatus = { state: 'connected', username: 'old', enabled: true,
  pendingCount: 0, retainedQueues: [], lastScrobbleAtMs: null, lastError: null, lastMetadataWarning: null }

beforeEach(() => {
  vi.resetAllMocks()
  useSessionStore.getState().clearSession()
  useListenStatsStore.getState().reset()
  useLastFmStore.setState({ status: null })
  useCacheStore.getState().clearAll()
})

describe('settings mutation lifetime', () => {
  it('does not restore disabled statistics from an old account', async () => {
    const response = deferred<void>()
    vi.mocked(api.setListenStatisticsEnabled).mockReturnValueOnce(response.promise)
    const pending = useListenStatsStore.getState().saveEnabled(false)
    const rejected = expect(pending).rejects.toThrow('account changed')
    await Promise.resolve()
    useSessionStore.getState().clearSession()
    useListenStatsStore.getState().reset()
    response.resolve()
    await rejected
    expect(useListenStatsStore.getState().enabled).toBe(true)
  })
  it('lets new-account settings finish while an old request is unresolved', async () => {
    const old = deferred<void>()
    vi.mocked(api.setListenStatisticsEnabled).mockReturnValueOnce(old.promise).mockResolvedValueOnce(undefined)
    const pending = useListenStatsStore.getState().saveEnabled(false).catch(() => {})
    await Promise.resolve()
    useSessionStore.getState().clearSession()
    await useListenStatsStore.getState().saveEnabled(true)
    old.resolve()
    await pending
    expect(useListenStatsStore.getState().enabled).toBe(true)
  })
  it('does not repopulate Last.fm after the account lifetime ends', async () => {
    const response = deferred<LastFmStatus>()
    vi.mocked(api.setLastFmEnabled).mockReturnValueOnce(response.promise)
    const pending = useLastFmStore.getState().saveEnabled(true)
    const rejected = expect(pending).rejects.toThrow('account changed')
    await Promise.resolve()
    useSessionStore.getState().clearSession()
    response.resolve(status)
    await rejected
    expect(useLastFmStore.getState().status).toBeNull()
  })
  it('keeps a status event ahead of a slow hydration snapshot', async () => {
    const response = deferred<LastFmStatus>()
    vi.mocked(api.getLastFmStatus).mockReturnValueOnce(response.promise)
    const pending = useLastFmStore.getState().hydrate()
    useLastFmStore.getState().setStatus({ ...status, pendingCount: 2 })
    response.resolve(status)
    await pending
    expect(useLastFmStore.getState().status?.pendingCount).toBe(2)
  })
  it('does not clear a new account cache after an old clear completes', async () => {
    const response = deferred<void>()
    vi.mocked(api.clearAudioCache).mockReturnValueOnce(response.promise)
    const pending = useCacheStore.getState().clearAudio()
    const rejected = expect(pending).rejects.toThrow('account changed')
    await Promise.resolve()
    useSessionStore.getState().clearSession()
    useCacheStore.getState().markCached([2])
    useCacheStore.getState().markBusy([2])
    response.resolve()
    await rejected
    expect(useCacheStore.getState().isCached(2)).toBe(true)
    expect(useCacheStore.getState().isBusy(2)).toBe(true)
    expect(api.getCacheUsage).not.toHaveBeenCalled()
  })
})
