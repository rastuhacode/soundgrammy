import { create } from 'zustand'
import { api, onCacheChanged, onDownloadProgress } from '@/lib/api'

interface CacheState {
  cachedIds: Set<number>
  busyIds: Set<number>
  progressById: Map<number, number>
  hydrated: boolean
  hydrate: () => Promise<void>
  markCached: (trackIds: number[]) => void
  markUncached: (trackIds: number[]) => void
  markBusy: (trackIds: number[]) => void
  clearBusy: (trackIds: number[]) => void
  setProgress: (trackId: number, ratio: number) => void
  clearProgress: (trackId: number) => void
  clearAll: () => void
  isCached: (trackId: number) => boolean
  isBusy: (trackId: number) => boolean
  isPlaylistCached: (trackIds: number[]) => boolean
}

export const useCacheStore = create<CacheState>((set, get) => ({
  cachedIds: new Set(),
  busyIds: new Set(),
  progressById: new Map(),
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

  markBusy: (trackIds) => {
    if (trackIds.length === 0) return
    set((state) => {
      const next = new Set(state.busyIds)
      for (const id of trackIds) next.add(id)
      return { busyIds: next }
    })
  },

  clearBusy: (trackIds) => {
    if (trackIds.length === 0) return
    set((state) => {
      const nextBusy = new Set(state.busyIds)
      const nextProgress = new Map(state.progressById)
      for (const id of trackIds) {
        nextBusy.delete(id)
        nextProgress.delete(id)
      }
      return { busyIds: nextBusy, progressById: nextProgress }
    })
  },

  setProgress: (trackId, ratio) => {
    const clamped = Math.min(1, Math.max(0, ratio))
    set((state) => {
      const next = new Map(state.progressById)
      next.set(trackId, clamped)
      return { progressById: next }
    })
  },

  clearProgress: (trackId) => {
    set((state) => {
      if (!state.progressById.has(trackId)) return state
      const next = new Map(state.progressById)
      next.delete(trackId)
      return { progressById: next }
    })
  },

  clearAll: () => set({
    cachedIds: new Set(),
    busyIds: new Set(),
    progressById: new Map(),
  }),

  isCached: trackId => get().cachedIds.has(trackId),

  isBusy: trackId => get().busyIds.has(trackId),

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

/** Subscribe once after login; drives thumbnail download progress. */
export function startDownloadProgressListener(): Promise<() => void> {
  return onDownloadProgress((progress) => {
    const store = useCacheStore.getState()
    if (progress.complete) {
      store.clearProgress(progress.trackId)
      return
    }
    if (progress.total <= 0) return
    store.setProgress(progress.trackId, progress.received / progress.total)
  })
}
