import { ownEventListeners } from '@/lib/events'
import { useEffect, useRef } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { api } from '@/lib/api'
import { startLastFmStatusListener, useLastFmStore } from '@/stores/lastfm-store'

/** Keeps safe Last.fm status current and completes desktop auth on refocus. */
export function useLastFmIntegration(enabled: boolean) {
  const completingRef = useRef(false)

  useEffect(() => {
    if (!enabled) return
    useLastFmStore.getState().hydrate().catch(() => {})
    const listener = startLastFmStatusListener()
    const focusListener = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (!focused || completingRef.current) return
      if (useLastFmStore.getState().status?.state !== 'waiting_for_browser_approval') return
      completingRef.current = true
      useLastFmStore.getState().completeAuth()
        .catch(() => {})
        .finally(() => {
          completingRef.current = false
        })
    })
    const unlisten = ownEventListeners([listener, focusListener])
    const onOnline = () => {
      api.flushLastFmQueue().catch(() => {})
    }
    window.addEventListener('online', onOnline)
    return () => {
      unlisten()
      window.removeEventListener('online', onOnline)
    }
  }, [enabled])
}
