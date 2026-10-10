import { create } from 'zustand'
import { api, type SyncProgress } from '@/lib/api'
import { errorMessage } from '@/lib/errors'
import { hydrateLibrary } from '@/lib/library-hydration'
import { captureSession, isSessionCurrent, assertSession, useSessionStore } from '@/stores/session-store'
import { authUserToSession } from '@/types'

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
  requestSync: () => Promise<void>
  setConnecting: () => void
  setOnline: () => void
  setOffline: () => void
  reset: () => void
}
let pending: { generation: number, promise: Promise<void> } | null = null

export const useConnectivityStore = create<ConnectivityState>((set, get) => ({
  phase: 'connecting', syncing: false, syncError: null, progress: null, lastSyncAt: undefined,
  setConnecting: () => set({ phase: 'connecting' }),
  setOnline: () => set({ phase: 'online' }),
  setOffline: () => set({ phase: 'offline' }),
  reset: () => set({ phase: 'connecting', syncing: false, syncError: null, progress: null, lastSyncAt: undefined }),
  requestSync: () => {
    const generation = captureSession()
    if (pending?.generation === generation) return pending.promise
    set({ syncing: true, syncError: null, progress: null })
    const promise = (async () => {
      await Promise.resolve()
      try {
        assertSession(generation)
        if (typeof navigator !== 'undefined' && !navigator.onLine) throw new Error('No network connection. Reconnect and try again.')
        if (get().phase !== 'online') {
          get().setConnecting()
          const auth = await api.refreshAuth()
          assertSession(generation)
          if (!auth.authorized || !auth.user) throw new Error('Your Telegram session is no longer authorized.')
          useSessionStore.getState().setSession(authUserToSession(auth.user))
        }
        assertSession(generation)
        const result = await api.syncSavedMusic()
        assertSession(generation)
        set({ phase: 'online', lastSyncAt: result.lastSyncAt, syncError: null })
        // This response is the only library reload path for automatic and manual sync.
        if (result.changed) await hydrateLibrary()
      }
      catch (error) {
        if (!isSessionCurrent(generation)) return
        const message = errorMessage(error)
        set({ syncError: message, phase: isOfflineError(message) ? 'offline' : get().phase })
        throw error
      }
      finally {
        if (isSessionCurrent(generation)) set({ syncing: false, progress: null })
        if (pending?.generation === generation) pending = null
      }
    })()
    pending = { generation, promise }
    return promise
  },
}))
