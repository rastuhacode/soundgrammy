import { useCallback, useEffect, useState } from 'react'
import { api } from '@/lib/api'
import { errorMessage } from '@/lib/errors'
import { assertSession, createSessionQueue } from '@/stores/session-store'
import type { ProxySettings, ProxySettingsView } from '@/types'
import { useAsyncScope } from '@/hooks/use-async-scope'

// Proxy changes can originate before login or from Settings; serialize both paths.
const enqueue = createSessionQueue()

/** Backend settings stay out of Zustand; one controller owns status and operations. */
export function useProxySettings(enabled = true) {
  const [view, setView] = useState<ProxySettingsView | null>(null)
  const [loaded, setLoaded] = useState(false)
  const [busy, setBusy] = useState(false)
  const [busyLabel, setBusyLabel] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const { capture } = useAsyncScope(enabled)

  const refresh = useCallback(async () => {
    const current = capture()
    try {
      const next = await enqueue(async (generation) => {
        const result = await api.getProxySettings()
        assertSession(generation)
        return result
      })
      if (current()) {
        setView(next)
        setError(null)
      }
    }
    catch (error) {
      if (current()) setError(errorMessage(error))
    }
    finally {
      if (current()) setLoaded(true)
    }
  }, [capture])

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Read backend settings for this controller lifetime.
    if (enabled) void refresh()
  }, [enabled, refresh])

  const apply = async (settings: ProxySettings) => {
    const current = capture()
    setBusy(true)
    setBusyLabel(settings.enabled
      ? 'Connecting via MTProto proxy (up to ~20s)…'
      : 'Reconnecting directly (up to ~20s)…')
    setError(null)
    try {
      const result = await enqueue(async (generation) => {
        try {
          const view = await api.setProxySettings(settings)
          assertSession(generation)
          return { view, error: null }
        }
        catch (error) {
          assertSession(generation)
          // Refresh actual settings without erasing the failed operation's error.
          const view = await api.getProxySettings().catch(() => null)
          assertSession(generation)
          return { view, error: errorMessage(error) }
        }
      })
      if (!current()) return
      if (result.view) setView(result.view)
      setError(result.error)
    }
    catch (error) {
      if (current()) setError(errorMessage(error))
    }
    finally {
      if (current()) {
        setBusy(false)
        setBusyLabel(null)
      }
    }
  }

  const parseLink = async (link: string): Promise<ProxySettings | null> => {
    const current = capture()
    setBusy(true)
    setError(null)
    try {
      const parsed = await api.parseProxyLink(link)
      return current() ? parsed : null
    }
    catch (error) {
      if (current()) setError(errorMessage(error))
      return null
    }
    finally {
      if (current()) setBusy(false)
    }
  }

  return { view, loaded, busy, busyLabel, error, refresh, apply, parseLink }
}

export type ProxySettingsController = ReturnType<typeof useProxySettings>
