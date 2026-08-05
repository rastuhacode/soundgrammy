import { useCallback, useEffect, useState } from 'react'
import {
  api,
  onSyncDone,
  onSyncError,
  onSyncProgress,
  onSyncStart,
  type SyncProgress,
} from '@/lib/api'
import { formatInvokeError } from '@/lib/playlist-recipe-io'
import { useConnectivityStore } from '@/stores/connectivity-store'

export type SyncPhase = 'connecting' | 'offline' | 'syncing' | 'live' | 'error'

const OFFLINE_ERROR_PATTERN
  = /offline|network|connect|timed? out|timeout|unreachable|transport|socket/i

function isOfflineError(message: string): boolean {
  return (typeof navigator !== 'undefined' && !navigator.onLine)
    || OFFLINE_ERROR_PATTERN.test(message)
}

function formatLastSync(value: string | null | undefined): string | null {
  if (!value) return null
  // Backend stores UTC "YYYY-MM-DD HH:MM:SS"; normalize to ISO for Date.
  const iso = value.includes('T') ? value : `${value.replace(' ', 'T')}Z`
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return null
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date)
}

export function useProfileMusicSync() {
  const connectivity = useConnectivityStore(state => state.phase)
  const [lastSyncAt, setLastSyncAt] = useState<string | null | undefined>(
    undefined,
  )
  const [syncing, setSyncing] = useState(false)
  const [manualSyncing, setManualSyncing] = useState(false)
  const [syncError, setSyncError] = useState<string | null>(null)
  const [progress, setProgress] = useState<SyncProgress | null>(null)

  useEffect(() => {
    let cancelled = false
    api
      .syncStatus()
      .then((value) => {
        if (!cancelled) setLastSyncAt(value)
      })
      .catch(() => {
        if (!cancelled) setLastSyncAt(null)
      })

    const startPromise = onSyncStart(() => {
      setSyncError(null)
      setProgress(null)
      setSyncing(true)
    })
    const progressPromise = onSyncProgress(setProgress)
    const donePromise = onSyncDone(() => {
      setSyncing(false)
      setSyncError(null)
      setProgress(null)
      useConnectivityStore.getState().setOnline()
      api
        .syncStatus()
        .then(value => setLastSyncAt(value))
        .catch(() => {})
    })
    const errorPromise = onSyncError((error) => {
      setSyncing(false)
      setSyncError(error.message)
      setProgress(null)
      if (isOfflineError(error.message)) {
        useConnectivityStore.getState().setOffline()
      }
    })

    return () => {
      cancelled = true
      startPromise.then(unlisten => unlisten())
      progressPromise.then(unlisten => unlisten())
      donePromise.then(unlisten => unlisten())
      errorPromise.then(unlisten => unlisten())
    }
  }, [])

  const requestSync = useCallback(async () => {
    if (syncing || manualSyncing) return

    setSyncError(null)
    setProgress(null)
    if (typeof navigator !== 'undefined' && !navigator.onLine) {
      useConnectivityStore.getState().setOffline()
      setSyncError('No network connection. Reconnect and try again.')
      return
    }

    setManualSyncing(true)
    try {
      if (useConnectivityStore.getState().phase === 'offline') {
        useConnectivityStore.getState().setConnecting()
        const auth = await api.refreshAuth()
        if (!auth.authorized) {
          throw new Error('Your Telegram session is no longer authorized.')
        }
        useConnectivityStore.getState().setOnline()
      }

      const result = await api.syncSavedMusic()
      setLastSyncAt(result.lastSyncAt)
      setSyncError(null)
      useConnectivityStore.getState().setOnline()
    }
    catch (error) {
      const message = formatInvokeError(error)
      setSyncError(message)
      if (isOfflineError(message)) {
        useConnectivityStore.getState().setOffline()
      }
    }
    finally {
      setManualSyncing(false)
    }
  }, [manualSyncing, syncing])

  const isSyncing = syncing || manualSyncing

  const phase: SyncPhase
    = connectivity === 'offline'
      ? 'offline'
      : connectivity === 'connecting' && !isSyncing
        ? 'connecting'
        : isSyncing
          ? 'syncing'
          : syncError
            ? 'error'
            : 'live'

  const lastSynced = formatLastSync(lastSyncAt)
  const lastSyncDetail
    = lastSyncAt === undefined
      ? 'Checking last sync…'
      : lastSynced
        ? `Last synced ${lastSynced}`
        : 'Not synced yet'

  const statusLabel
    = phase === 'connecting'
      ? 'connecting'
      : phase === 'offline'
        ? 'offline'
        : phase === 'syncing'
          ? 'syncing'
          : phase === 'error'
            ? 'sync failed'
            : 'live'

  const statusDetail
    = phase === 'connecting'
      ? 'Connecting to Telegram…'
      : phase === 'offline'
        ? syncError ?? 'Waiting for network…'
        : phase === 'syncing'
          ? progress && progress.total > 0
            ? `Pulling ${progress.done} of ${progress.total} tracks…`
            : 'Pulling your library…'
          : phase === 'error'
            ? syncError ?? 'Could not sync with Telegram.'
            : lastSyncDetail

  return {
    phase,
    statusLabel,
    statusDetail,
    lastSyncDetail,
    requestSync,
    isSyncing,
  }
}
