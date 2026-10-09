import { assertSession, captureSession, createSessionQueue, isSessionCurrent } from '@/stores/session-store'
import { create } from 'zustand'
import { api, onCacheChanged, onDownloadProgress } from '@/lib/api'
import type { CacheSettings, CacheUsage } from '@/types'

interface CacheState {
  cachedIds: Set<number>
  busyIds: Set<number>
  /** Refcount per track so overlapping jobs do not clear each other's busy state. */
  busyCounts: Map<number, number>
  progressById: Map<number, number>
  revision: number
  clearEpoch: number
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
  saveSettings: (settings: CacheSettings) => Promise<{ settings: CacheSettings, usage: CacheUsage }>
  clearAudio: () => Promise<CacheUsage>
}

const mutate = createSessionQueue()
let snapshotRequest = 0
const changedAt = new Map<number, number>()

/** Shared busy ownership for track operations and playlist jobs. */
export async function withBusyTracks<T>(trackIds: number[], operation: (generation: number) => Promise<T>): Promise<T> {
  const generation = captureSession()
  useCacheStore.getState().markBusy(trackIds)
  try {
    const result = await operation(generation)
    assertSession(generation)
    return result
  }
  finally {
    if (isSessionCurrent(generation)) useCacheStore.getState().clearBusy(trackIds)
  }
}

export const useCacheStore = create<CacheState>((set, get) => ({
  cachedIds: new Set(),
  busyIds: new Set(),
  busyCounts: new Map(),
  progressById: new Map(),
  revision: 0,
  clearEpoch: 0,
  hydrated: false,

  saveSettings: settings => mutate(async (generation) => {
    const next = await api.setCacheSettings(settings)
    assertSession(generation)
    const usage = await api.getCacheUsage()
    assertSession(generation)
    await get().hydrate()
    assertSession(generation)
    return { settings: next, usage }
  }),
  clearAudio: () => mutate(async (generation) => {
    await api.clearAudioCache()
    assertSession(generation)
    get().clearAll()
    const usage = await api.getCacheUsage()
    assertSession(generation)
    return usage
  }),

  hydrate: async () => {
    const generation = captureSession()
    const { revision, clearEpoch } = get()
    const request = ++snapshotRequest
    const current = () => isSessionCurrent(generation) && get().clearEpoch === clearEpoch && request === snapshotRequest
    try {
      const ids = await api.getCacheStatus()
      if (!current()) return
      const latest = get()
      const cachedIds = new Set(ids)
      // Replay live additions/removals, including removals absent from the old mirror.
      for (const [id, changedRevision] of changedAt) {
        if (changedRevision <= revision) continue
        if (latest.cachedIds.has(id)) cachedIds.add(id)
        else cachedIds.delete(id)
      }
      set({ cachedIds, hydrated: true, revision: latest.revision + 1 })
    }
    catch {
      if (current()) set({ hydrated: true })
    }
  },

  markCached: (trackIds) => {
    if (trackIds.length === 0) return
    set((state) => {
      const next = new Set(state.cachedIds)
      for (const id of trackIds) {
        next.add(id)
        changedAt.set(id, state.revision + 1)
      }
      return { cachedIds: next, revision: state.revision + 1 }
    })
  },

  markUncached: (trackIds) => {
    if (trackIds.length === 0) return
    set((state) => {
      const next = new Set(state.cachedIds)
      for (const id of trackIds) {
        next.delete(id)
        changedAt.set(id, state.revision + 1)
      }
      return { cachedIds: next, revision: state.revision + 1 }
    })
  },

  markBusy: (trackIds) => {
    if (trackIds.length === 0) return
    set((state) => {
      const nextBusy = new Set(state.busyIds)
      const nextCounts = new Map(state.busyCounts)
      for (const id of trackIds) {
        const count = (nextCounts.get(id) ?? 0) + 1
        nextCounts.set(id, count)
        nextBusy.add(id)
      }
      return { busyIds: nextBusy, busyCounts: nextCounts }
    })
  },

  clearBusy: (trackIds) => {
    if (trackIds.length === 0) return
    set((state) => {
      const nextBusy = new Set(state.busyIds)
      const nextCounts = new Map(state.busyCounts)
      const nextProgress = new Map(state.progressById)
      for (const id of trackIds) {
        const count = (nextCounts.get(id) ?? 0) - 1
        if (count <= 0) {
          nextCounts.delete(id)
          nextBusy.delete(id)
          nextProgress.delete(id)
        }
        else {
          nextCounts.set(id, count)
        }
      }
      return {
        busyIds: nextBusy,
        busyCounts: nextCounts,
        progressById: nextProgress,
      }
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

  clearAll: () => {
    changedAt.clear()
    set(state => ({
      revision: state.revision + 1,
      clearEpoch: state.clearEpoch + 1,
      hydrated: false,
      cachedIds: new Set(),
      busyIds: new Set(),
      busyCounts: new Map(),
      progressById: new Map(),
    }))
  },

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
  const generation = captureSession()
  return onCacheChanged((payload) => {
    if (!isSessionCurrent(generation)) return
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
  const generation = captureSession()
  return onDownloadProgress((progress) => {
    if (!isSessionCurrent(generation)) return
    const store = useCacheStore.getState()
    if (progress.complete) {
      store.clearProgress(progress.trackId)
      return
    }
    if (progress.total <= 0) return
    store.setProgress(progress.trackId, progress.received / progress.total)
  })
}
