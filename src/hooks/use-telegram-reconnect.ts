import { useEffect, useRef } from 'react'
import { api } from '@/lib/api'
import type { AuthUser } from '@/types'
import { useConnectivityStore } from '@/stores/connectivity-store'
import {
  createBackoffState,
  createTimerSleep,
  runReconnectLoop,
} from '@/lib/telegram-reconnect'

/**
 * Telegram-like reconnect: while the app is ready, keep trying `refresh_auth`
 * with exponential backoff until Telegram is reachable. Browser `online`
 * wakes the wait immediately; `offline` drops back into the reconnect loop.
 */
export function useTelegramReconnect(options: {
  enabled: boolean
  onUser: (user: AuthUser) => void
  onConnected: () => Promise<void>
}) {
  const { enabled, onUser, onConnected } = options
  const onUserRef = useRef(onUser)
  const onConnectedRef = useRef(onConnected)

  useEffect(() => {
    onUserRef.current = onUser
    onConnectedRef.current = onConnected
  }, [onUser, onConnected])

  useEffect(() => {
    if (!enabled) return

    let cancelled = false
    let activeWake: (() => void) | null = null
    const backoff = createBackoffState()
    const sleep = createTimerSleep()

    const wrappedSleep: typeof sleep = (ms) => {
      const handle = sleep(ms)
      activeWake = handle.wake
      return {
        promise: handle.promise.finally(() => {
          if (activeWake === handle.wake) activeWake = null
        }),
        wake: handle.wake,
      }
    }

    const bumpWake = () => {
      activeWake?.()
    }

    const onBrowserOnline = () => {
      backoff.reset()
      if (useConnectivityStore.getState().phase === 'online') return
      useConnectivityStore.getState().setConnecting()
      bumpWake()
    }

    const onBrowserOffline = () => {
      useConnectivityStore.getState().setOffline()
      bumpWake()
    }

    window.addEventListener('online', onBrowserOnline)
    window.addEventListener('offline', onBrowserOffline)

    runReconnectLoop({
      refreshAuth: () => api.refreshAuth(),
      getPhase: () => useConnectivityStore.getState().phase,
      setPhase: (phase) => {
        const store = useConnectivityStore.getState()
        if (phase === 'connecting') store.setConnecting()
        else if (phase === 'online') store.setOnline()
        else store.setOffline()
      },
      onUser: user => onUserRef.current(user),
      onConnected: () => onConnectedRef.current(),
      backoff,
      sleep: wrappedSleep,
      isCancelled: () => cancelled,
    })

    return () => {
      cancelled = true
      bumpWake()
      window.removeEventListener('online', onBrowserOnline)
      window.removeEventListener('offline', onBrowserOffline)
    }
  }, [enabled])
}
