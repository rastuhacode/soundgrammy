import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { hydrateLibrary } from '@/lib/library-hydration'
import { useConnectivityStore } from './connectivity-store'
import { useSessionStore } from './session-store'
import { deferred } from '@/test-support/fixtures'
import type { SyncResult } from '@/types'
vi.mock('@/lib/api', () => ({ api: { syncSavedMusic: vi.fn(), refreshAuth: vi.fn() } }))
vi.mock('@/lib/library-hydration', () => ({ hydrateLibrary: vi.fn(async () => {}) }))
beforeEach(() => {
  vi.clearAllMocks()
  vi.stubGlobal('navigator', { onLine: true })
  useSessionStore.getState().clearSession()
  useConnectivityStore.getState().reset()
  useConnectivityStore.getState().setOnline()
})
describe('sync coordination', () => {
  it('deduplicates simultaneous manual and reconnect requests and reloads once', async () => {
    const pending = deferred<SyncResult>()
    vi.mocked(api.syncSavedMusic).mockReturnValue(pending.promise)
    const automatic = useConnectivityStore.getState().requestSync()
    const manual = useConnectivityStore.getState().requestSync()
    expect(automatic).toBe(manual)
    await Promise.resolve()
    expect(api.syncSavedMusic).toHaveBeenCalledTimes(1)
    pending.resolve({ changed: true, total: 3, lastSyncAt: 'now' })
    await automatic
    expect(hydrateLibrary).toHaveBeenCalledTimes(1)
    expect(useConnectivityStore.getState().syncing).toBe(false)
  })
  it('ignores a completed sync from a cleared session', async () => {
    const pending = deferred<SyncResult>()
    vi.mocked(api.syncSavedMusic).mockReturnValue(pending.promise)
    const request = useConnectivityStore.getState().requestSync()
    await Promise.resolve()
    useSessionStore.getState().clearSession()
    useConnectivityStore.getState().reset()
    pending.resolve({ changed: true, total: 3, lastSyncAt: 'old' })
    await request
    expect(hydrateLibrary).not.toHaveBeenCalled()
    expect(useConnectivityStore.getState().lastSyncAt).toBeUndefined()
  })
  it('propagates network failures so the reconnect loop applies backoff', async () => {
    vi.mocked(api.syncSavedMusic).mockRejectedValue('network unavailable')
    await expect(useConnectivityStore.getState().requestSync()).rejects.toBe('network unavailable')
    expect(useConnectivityStore.getState().phase).toBe('offline')
    expect(useConnectivityStore.getState().syncError).toBe('network unavailable')
  })
})
