import { create } from 'zustand'
import { api, type SyncProgress } from '@/lib/api'
import { errorMessage } from '@/lib/errors'
import { hydrateLibrary } from '@/lib/library-hydration'
import { captureSession, isSessionCurrent, assertSession, useSessionStore } from '@/stores/session-store'
import { authUserToSession } from '@/types'
import type { SyncMode } from '@/types'
import { useLibraryStore } from '@/stores/library-store'
import { usePlaylistsStore } from '@/stores/playlists-store'

export type ConnectivityPhase = 'connecting' | 'online' | 'offline'
export function isOfflineError(message: string): boolean {
  return (typeof navigator !== 'undefined' && !navigator.onLine)
    || /offline|network|connect|timed? out|timeout|unreachable|transport|socket/i.test(message)
}
interface ConnectivityState {
  phase: ConnectivityPhase
  syncing: boolean
  syncError: string | null
  progress: SyncProgress | null
  lastSyncAt: string | null | undefined
  blockingIndex: boolean
  syncMode: SyncMode
  requestSync: (mode?: SyncMode) => Promise<void>
  setConnecting: () => void
  setOnline: () => void
  setOffline: () => void
  reset: () => void
}
let pending: { generation: number, mode: SyncMode, promise: Promise<void> } | null = null

export const useConnectivityStore = create<ConnectivityState>((set, get) => ({
  phase: 'connecting', syncing: false, syncError: null, progress: null, lastSyncAt: undefined,
  blockingIndex: false, syncMode: 'soft',
  setConnecting: () => set({ phase: 'connecting' }),
  setOnline: () => set({ phase: 'online' }),
  setOffline: () => set({ phase: 'offline' }),
  reset: () => set({ phase: 'connecting', syncing: false, syncError: null, progress: null, lastSyncAt: undefined,
    blockingIndex: false, syncMode: 'soft' }),
  requestSync: (mode = 'soft') => {
    const generation = captureSession()
    if (pending?.generation === generation) {
      if (mode === 'soft' || pending.mode === 'full') return pending.promise
      // A confirmed full request must not be swallowed by a running soft sync.
      return pending.promise.catch(() => {}).then(() => {
        assertSession(generation)
        return get().requestSync('full')
      })
    }
    const blockingIndex = get().blockingIndex || mode === 'full' || useLibraryStore.getState().status?.blocking === true
    set({ syncing: true, syncMode: mode, blockingIndex, syncError: null,
      progress: blockingIndex
        ? { done: mode === 'full' ? 0 : useLibraryStore.getState().status?.processed ?? 0,
            total: useLibraryStore.getState().status?.total ?? 0 }
        : null })
    const promise = (async () => {
      await Promise.resolve()
      try {
        assertSession(generation)
        if (mode === 'full') {
          // Persist the user's confirmation even if Telegram is offline.
          const batch = await api.prepareFullReindex()
          assertSession(generation)
          useLibraryStore.getState().applyBatch(batch)
        }
        if (typeof navigator !== 'undefined' && !navigator.onLine) throw new Error('No network connection. Reconnect and try again.')
        if (get().phase !== 'online') {
          get().setConnecting()
          const auth = await api.refreshAuth()
          assertSession(generation)
          if (!auth.authorized || !auth.user) throw new Error('Your Telegram session is no longer authorized.')
          useSessionStore.getState().setSession(authUserToSession(auth.user))
        }
        assertSession(generation)
        const result = await api.syncSavedMusic(mode)
        assertSession(generation)
        set({ phase: 'online', lastSyncAt: result.lastSyncAt, syncError: null })
        // This response is the only library reload path for automatic and manual sync.
        if (result.changed || get().blockingIndex) await hydrateLibrary()
        assertSession(generation)
        if (get().blockingIndex) {
          const library = useLibraryStore.getState()
          if (!library.status?.complete || library.status.blocking || library.error || usePlaylistsStore.getState().error) {
            throw new Error('Could not load the indexed library. Please retry.')
          }
          set({ blockingIndex: false })
        }
      }
      catch (error) {
        if (!isSessionCurrent(generation)) return
        const message = errorMessage(error)
        set({ syncError: message, phase: isOfflineError(message) ? 'offline' : get().phase })
        // A failed remote page can follow successful durable batches. Reload
        // those locally, including if the WebView missed their events.
        await hydrateLibrary()
        throw error
      }
      finally {
        if (isSessionCurrent(generation)) set({ syncing: false, progress: null })
        if (pending?.generation === generation) pending = null
      }
    })()
    pending = { generation, mode, promise }
    return promise
  },
}))
