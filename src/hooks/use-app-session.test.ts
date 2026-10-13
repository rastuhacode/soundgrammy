// @vitest-environment jsdom
import { act, createElement, StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useAppSession, resetSessionState } from './use-app-session'
import { api, onAuthRevoked, onLibraryBatch } from '@/lib/api'
import { useLibraryStore } from '@/stores/library-store'
import { useSessionStore } from '@/stores/session-store'
import { usePlaylistJobsStore } from '@/stores/playlist-jobs-store'
import { deferred, libraryPage, playlists, track } from '@/test-support/fixtures'
import type { AuthStatus, LibraryPage } from '@/types'
vi.mock('@/hooks/use-telegram-reconnect', () => ({ useTelegramReconnect: vi.fn() }))
vi.mock('@/hooks/use-lastfm-integration', () => ({ useLastFmIntegration: vi.fn() }))
vi.mock('@/lib/api', () => ({ api: {
  authStatus: vi.fn(), listTracksPage: vi.fn(), listPlaylists: vi.fn(), getListenStatisticsEnabled: vi.fn(), listListenStats: vi.fn(),
  getCacheStatus: vi.fn(), nativePlayerCommand: vi.fn(async () => undefined), nativeAudioSnapshot: vi.fn(async () => undefined), syncStatus: vi.fn(async () => null),
},
onAuthRevoked: vi.fn(async () => () => {}), onSyncStart: vi.fn(async () => () => {}), onSyncProgress: vi.fn(async () => () => {}),
onLibraryBatch: vi.fn(async () => () => {}),
onCacheChanged: vi.fn(async () => () => {}), onDownloadProgress: vi.fn(async () => () => {}), onPlaylistDownloadProgress: vi.fn(async () => () => {}), onCacheTracksProgress: vi.fn(async () => () => {}),
}))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
const authorized: AuthStatus = { authorized: true, user: { id: 1, firstName: 'Test', lastName: null, username: null, phone: null } }
beforeEach(() => {
  vi.clearAllMocks()
  resetSessionState()
  vi.mocked(api.authStatus).mockResolvedValue(authorized)
  vi.mocked(api.listTracksPage).mockResolvedValue(libraryPage([track(1)]))
  vi.mocked(api.listPlaylists).mockResolvedValue(playlists())
  vi.mocked(api.getListenStatisticsEnabled).mockResolvedValue(true)
  vi.mocked(api.listListenStats).mockResolvedValue([])
  vi.mocked(api.getCacheStatus).mockResolvedValue([])
})
describe('application session lifetime', () => {
  it('does not re-enter ready when revoked during initial library hydration', async () => {
    const tracks = deferred<LibraryPage>()
    vi.mocked(api.listTracksPage).mockReturnValue(tracks.promise)
    let model!: ReturnType<typeof useAppSession>
    function Probe() {
      model = useAppSession()
      return null
    }
    const root = createRoot(document.createElement('div'))
    try {
      await act(async () => root.render(createElement(Probe)))
      expect(model.status).toBe('loading')
      const revoke = vi.mocked(onAuthRevoked).mock.calls.at(-1)![0]
      await act(async () => revoke())
      tracks.resolve(libraryPage([track(1)]))
      await act(async () => await tracks.promise)
      expect(model.status).toBe('login')
      expect(useSessionStore.getState().session).toBeNull()
      expect(useLibraryStore.getState().library).toEqual([])
    }
    finally { await act(async () => root.unmount()) }
  })
  it('ignores the discarded bootstrap in Strict Mode', async () => {
    const old = deferred<AuthStatus>()
    vi.mocked(api.authStatus).mockReturnValueOnce(old.promise).mockResolvedValueOnce(authorized)
    let model!: ReturnType<typeof useAppSession>
    function Probe() {
      model = useAppSession()
      return null
    }
    const root = createRoot(document.createElement('div'))
    try {
      await act(async () => root.render(createElement(StrictMode, null, createElement(Probe))))
      expect(model.status).toBe('ready')
      old.resolve({ authorized: false, user: null })
      await act(async () => await old.promise)
      expect(model.status).toBe('ready')
      expect(useLibraryStore.getState().library[0]?.id).toBe(1)
    }
    finally { await act(async () => root.unmount()) }
  })
  it('clears outstanding job presentation on logout', () => {
    usePlaylistJobsStore.getState().enqueueError('old account')
    resetSessionState()
    expect(usePlaylistJobsStore.getState().errorQueue).toEqual([])
  })

  it('publishes remote pages before sync completion and discards callbacks after logout', async () => {
    let model!: ReturnType<typeof useAppSession>
    function Probe() {
      model = useAppSession()
      return null
    }
    const root = createRoot(document.createElement('div'))
    try {
      await act(async () => root.render(createElement(Probe)))
      const callback = vi.mocked(onLibraryBatch).mock.calls.at(-1)![0]
      const page = libraryPage([track(2)], 1, null, 2)
      page.status.complete = false
      page.status.total = 10000
      await act(async () => callback({ ...page, removedTrackIds: [] }))
      expect(useLibraryStore.getState().library.map(track => track.id)).toEqual([1, 2])
      expect(useLibraryStore.getState().status?.complete).toBe(false)
      await act(async () => model.resetToLogin())
      await act(async () => callback({ ...libraryPage([track(3)], 2, null, 3), removedTrackIds: [] }))
      expect(useLibraryStore.getState().library).toEqual([])
    }
    finally { await act(async () => root.unmount()) }
  })
})
