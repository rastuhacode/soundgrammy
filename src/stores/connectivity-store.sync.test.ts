import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { hydrateLibrary } from '@/lib/library-hydration'
import { useConnectivityStore } from './connectivity-store'
import { useSessionStore } from './session-store'
import { deferred, libraryPage, track } from '@/test-support/fixtures'
import { useLibraryStore } from './library-store'
import type { SyncResult } from '@/types'
vi.mock('@/lib/api', () => ({ api: { syncSavedMusic: vi.fn(), prepareFullReindex: vi.fn(), refreshAuth: vi.fn() } }))
vi.mock('@/lib/library-hydration', () => ({ hydrateLibrary: vi.fn(async () => {}) }))
beforeEach(() => {
  vi.resetAllMocks()
  vi.mocked(hydrateLibrary).mockResolvedValue(undefined)
  vi.stubGlobal('navigator', { onLine: true })
  useSessionStore.getState().clearSession()
  useLibraryStore.getState().setLibrary([])
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
    expect(hydrateLibrary).toHaveBeenCalledTimes(1)
  })
  it('keeps full indexing blocked through the final local snapshot load', async () => {
    useSessionStore.getState().setSession({ tgUserId: 1, firstName: 'Test', lastName: null, username: null })
    useLibraryStore.getState().setLibrary([track(1)])
    const full = { ...libraryPage([]).status, revision: 1, complete: false, blocking: true, total: 3 }
    vi.mocked(api.prepareFullReindex).mockResolvedValue({ tracks: [], removedTrackIds: [], status: full })
    vi.mocked(api.syncSavedMusic).mockResolvedValue({ changed: true, total: 3, lastSyncAt: 'now' })
    const loading = deferred<void>()
    vi.mocked(hydrateLibrary).mockImplementation(async () => {
      await loading.promise
      useLibraryStore.setState({ status: { ...full, revision: 3, blocking: false, complete: true } })
    })
    const request = useConnectivityStore.getState().requestSync('full')
    await vi.waitFor(() => expect(hydrateLibrary).toHaveBeenCalledTimes(1))
    expect(api.prepareFullReindex).toHaveBeenCalledTimes(1)
    expect(api.syncSavedMusic).toHaveBeenCalledExactlyOnceWith('full')
    expect(useConnectivityStore.getState().blockingIndex).toBe(true)
    expect(useLibraryStore.getState().library).toEqual([])
    loading.resolve()
    await request
    expect(useConnectivityStore.getState().blockingIndex).toBe(false)
  })
  it('persists full-reindex intent even offline and leaves the loader active after failure', async () => {
    useSessionStore.getState().setSession({ tgUserId: 1, firstName: 'Test', lastName: null, username: null })
    useLibraryStore.getState().setLibrary([track(1)])
    vi.stubGlobal('navigator', { onLine: false })
    const full = { ...libraryPage([]).status, revision: 1, complete: false, blocking: true }
    vi.mocked(api.prepareFullReindex).mockResolvedValue({ tracks: [], removedTrackIds: [], status: full })
    await expect(useConnectivityStore.getState().requestSync('full')).rejects.toThrow('No network connection')
    expect(api.prepareFullReindex).toHaveBeenCalledTimes(1)
    expect(api.syncSavedMusic).not.toHaveBeenCalled()
    expect(useConnectivityStore.getState().blockingIndex).toBe(true)
    expect(useLibraryStore.getState().status?.blocking).toBe(true)
    expect(useLibraryStore.getState().library).toEqual([])
  })
  it('resumes a persisted full index via soft sync without clearing its checkpoint again', async () => {
    const partial = { ...libraryPage([]).status, revision: 1, complete: false, blocking: true, processed: 100, total: 203 }
    useLibraryStore.setState({ status: partial })
    const remote = deferred<SyncResult>()
    vi.mocked(api.syncSavedMusic).mockReturnValue(remote.promise)
    vi.mocked(hydrateLibrary).mockImplementation(async () => {
      useLibraryStore.setState({ status: { ...partial, complete: true, blocking: false, processed: 203 } })
    })
    const request = useConnectivityStore.getState().requestSync('soft')
    expect(useConnectivityStore.getState().blockingIndex).toBe(true)
    expect(useConnectivityStore.getState().progress?.done).toBe(100)
    remote.resolve({ changed: true, total: 203, lastSyncAt: 'now' })
    await request
    expect(api.prepareFullReindex).not.toHaveBeenCalled()
    expect(api.syncSavedMusic).toHaveBeenCalledExactlyOnceWith('soft')
    expect(useConnectivityStore.getState().blockingIndex).toBe(false)
  })
  it('keeps ordinary soft sync visible and never prepares a full reindex', async () => {
    const pending = deferred<SyncResult>()
    vi.mocked(api.syncSavedMusic).mockReturnValue(pending.promise)
    useLibraryStore.setState({ status: libraryPage([track(1)]).status })
    const request = useConnectivityStore.getState().requestSync('soft')
    expect(useConnectivityStore.getState().blockingIndex).toBe(false)
    pending.resolve({ changed: true, total: 2, lastSyncAt: 'now' })
    await request
    expect(api.prepareFullReindex).not.toHaveBeenCalled()
    expect(api.syncSavedMusic).toHaveBeenCalledExactlyOnceWith('soft')
    expect(useConnectivityStore.getState().blockingIndex).toBe(false)
  })
  it('queues a full request behind a running soft sync instead of dropping it', async () => {
    const soft = deferred<SyncResult>()
    vi.mocked(api.syncSavedMusic).mockReturnValueOnce(soft.promise)
      .mockResolvedValueOnce({ changed: true, total: 3, lastSyncAt: 'now' })
    vi.mocked(api.prepareFullReindex).mockResolvedValue({ tracks: [], removedTrackIds: [],
      status: { ...libraryPage([]).status, complete: false, blocking: true } })
    vi.mocked(hydrateLibrary).mockImplementation(async () => {
      useLibraryStore.setState({ status: libraryPage([]).status })
    })
    const first = useConnectivityStore.getState().requestSync('soft')
    const full = useConnectivityStore.getState().requestSync('full')
    soft.resolve({ changed: false, total: 3, lastSyncAt: 'now' })
    await Promise.all([first, full])
    expect(vi.mocked(api.syncSavedMusic).mock.calls.map(([mode]) => mode)).toEqual(['soft', 'full'])
    expect(api.prepareFullReindex).toHaveBeenCalledTimes(1)
  })
})
