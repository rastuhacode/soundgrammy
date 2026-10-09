import { useCallback, useEffect, useState } from 'react'
import { api, onAuthRevoked, onSyncStart, onSyncProgress } from '@/lib/api'
import { ownEventListeners } from '@/lib/events'
import { hydrateLibrary } from '@/lib/library-hydration'
import { authUserToSession, type AuthUser } from '@/types'
import { captureSession, isSessionCurrent, useSessionStore } from '@/stores/session-store'
import { useLibraryStore } from '@/stores/library-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { usePlayerStore } from '@/stores/player-store'
import { startCacheStatusListener, startDownloadProgressListener, useCacheStore } from '@/stores/cache-store'
import { startPlaylistJobsListeners, usePlaylistJobsStore } from '@/stores/playlist-jobs-store'
import { useFullscreenStore } from '@/stores/fullscreen-store'
import { useLastFmStore } from '@/stores/lastfm-store'
import { useConnectivityStore } from '@/stores/connectivity-store'
import { clearThumbnailMemoryCache } from '@/hooks/use-cached-thumbnail'
import { useTelegramReconnect } from '@/hooks/use-telegram-reconnect'
import { useLastFmIntegration } from '@/hooks/use-lastfm-integration'

export type AppStatus = 'loading' | 'login' | 'ready'

export function resetSessionState() {
  // Invalidate outstanding work before clearing any mirrors.
  useSessionStore.getState().clearSession()
  void useFullscreenStore.getState().exitFullscreen()
  useLibraryStore.getState().setLibrary([])
  usePlaylistsStore.getState().reset()
  useListenStatsStore.getState().reset()
  useCacheStore.getState().clearAll()
  usePlaylistJobsStore.getState().reset()
  useLastFmStore.setState({ status: null })
  clearThumbnailMemoryCache()
  usePlayerStore.getState().clearQueue()
  useConnectivityStore.getState().reset()
}

/** Auth, hydration and backend subscriptions share one account lifetime. */
export function useAppSession() {
  const [status, setStatus] = useState<AppStatus>('loading')
  const session = useSessionStore(state => state.session)
  const resetToLogin = useCallback(() => {
    resetSessionState()
    setStatus('login')
  }, [])
  const onUser = useCallback((user: AuthUser) => {
    useSessionStore.getState().setSession(authUserToSession(user))
  }, [])
  const onConnected = useCallback(() => useConnectivityStore.getState().requestSync(), [])
  useTelegramReconnect({ enabled: status === 'ready', onUser, onConnected })
  useLastFmIntegration(status === 'ready')

  useEffect(() => {
    let active = true
    let generation = captureSession()
    void (async () => {
      try {
        const auth = await api.authStatus()
        if (!active || !isSessionCurrent(generation)) return
        if (!auth.authorized || !auth.user) {
          setStatus('login')
          return
        }
        onUser(auth.user)
        generation = captureSession()
        await hydrateLibrary(true)
        if (!active || !isSessionCurrent(generation)) return
        useConnectivityStore.getState().setConnecting()
        setStatus('ready')
      }
      catch {
        if (active && isSessionCurrent(generation)) setStatus('login')
      }
    })()
    return () => {
      active = false
    }
  }, [onUser])

  useEffect(() => ownEventListeners([onAuthRevoked(resetToLogin)]), [resetToLogin])
  useEffect(() => {
    if (status !== 'ready') return
    const generation = captureSession()
    let active = true
    const current = () => active && isSessionCurrent(generation)
    void api.syncStatus().then((lastSyncAt) => {
      if (current() && useConnectivityStore.getState().lastSyncAt === undefined) useConnectivityStore.setState({ lastSyncAt })
    }).catch(() => {})
    const unlisten = ownEventListeners([
      onSyncStart(() => { if (current()) useConnectivityStore.setState({ progress: null }) }),
      onSyncProgress((progress) => { if (current()) useConnectivityStore.setState({ progress }) }),
      startCacheStatusListener(), startDownloadProgressListener(), startPlaylistJobsListeners(),
    ])
    return () => {
      active = false
      unlisten()
    }
  }, [status])

  const handleAuthenticated = useCallback(async (user: AuthUser) => {
    onUser(user)
    const generation = captureSession()
    useConnectivityStore.getState().setOnline()
    await hydrateLibrary(true)
    if (!isSessionCurrent(generation)) return
    setStatus('ready')
    void onConnected().catch(() => {})
  }, [onUser, onConnected])

  return { status, session, handleAuthenticated, resetToLogin }
}
