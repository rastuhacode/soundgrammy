import { useConnectivityStore } from '@/stores/connectivity-store'

export type SyncPhase = 'connecting' | 'offline' | 'syncing' | 'live' | 'error'

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
  const lastSyncAt = useConnectivityStore(state => state.lastSyncAt)
  const isSyncing = useConnectivityStore(state => state.syncing)
  const syncError = useConnectivityStore(state => state.syncError)
  const progress = useConnectivityStore(state => state.progress)
  const sync = useConnectivityStore(state => state.requestSync)
  const requestSync = () => sync().catch(() => {})

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
            : 'online'

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
