import { create } from 'zustand'
import { api, onCacheChanged } from '@/lib/api'

interface CacheState {
  cachedIds: Set<number>
  hydrated: boolean
  hydrate: () => Promise<void>
  markCached: (trackIds: number[]) => void
  markUncached: (trackIds: number[]) => void
  clearAll: () => void
  isCached: (trackId: number) => boolean
  isPlaylistCached: (trackIds: number[]) => boolean
}

export const useCacheStore = create<CacheState>((set, get) => ({
  cachedIds: new Set(),
  hydrated: false,

  hydrate: async () => {
    try {
      const ids = await api.getCacheStatus()
      set({ cachedIds: new Set(ids), hydrated: true })
    }
    catch {
      set({ hydrated: true })
    }
  },

  markCached: (trackIds) => {
    if (trackIds.length === 0) return
    set((state) => {
      const next = new Set(state.cachedIds)
      for (const id of trackIds) next.add(id)
      return { cachedIds: next }
    })
  },

  markUncached: (trackIds) => {
    if (trackIds.length === 0) return
    set((state) => {
      const next = new Set(state.cachedIds)
      for (const id of trackIds) next.delete(id)
      return { cachedIds: next }
    })
  },

  clearAll: () => set({ cachedIds: new Set() }),

  isCached: trackId => get().cachedIds.has(trackId),

  isPlaylistCached: (trackIds) => {
    if (trackIds.length === 0) return false
    const cached = get().cachedIds
    return trackIds.every(id => cached.has(id))
  },
}))

/** Subscribe once after login; updates borders when cache changes. */
export function startCacheStatusListener(): Promise<() => void> {
  return onCacheChanged((payload) => {
    const store = useCacheStore.getState()
    if (payload.cleared) {
      store.clearAll()
      return
    }
    if (payload.cached) {
      store.markCached(payload.trackIds)
    }
    else {
      store.markUncached(payload.trackIds)
    }
  })
}
