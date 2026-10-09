import { api } from '@/lib/api'
import { captureSession, isSessionCurrent } from '@/stores/session-store'
import { create } from 'zustand'
import type { TrackListenStats } from '@/types'

export * from '@/lib/listen-stats'

function statsToMap(
  stats: TrackListenStats[],
): Map<number, TrackListenStats> {
  const next = new Map<number, TrackListenStats>()
  for (const row of stats) {
    next.set(row.track_id, row)
  }
  return next
}

interface ListenStatsState {
  enabled: boolean
  revision: number
  settingsRevision: number
  clearEpoch: number
  statsByTrackId: Map<number, TrackListenStats>
  refresh: () => Promise<void>
  reset: () => void
  hydrate: (enabled: boolean, stats: TrackListenStats[]) => void
  setEnabled: (enabled: boolean) => void
  upsert: (stats: TrackListenStats) => void
  clear: () => void
}

export const useListenStatsStore = create<ListenStatsState>((set, get) => ({
  revision: 0,
  settingsRevision: 0,
  enabled: true,
  clearEpoch: 0,
  statsByTrackId: new Map(),

  reset: () => {
    get().clear()
    get().setEnabled(true)
  },
  refresh: async () => {
    const generation = captureSession()
    const { revision, settingsRevision } = get()
    await Promise.allSettled([
      api.getListenStatisticsEnabled().then((enabled) => {
        if (isSessionCurrent(generation) && get().settingsRevision === settingsRevision) get().setEnabled(enabled)
      }),
      api.listListenStats().then((stats) => {
        // A live upsert or clear after request dispatch wins over this snapshot.
        if (isSessionCurrent(generation) && get().revision === revision) {
          set({ statsByTrackId: statsToMap(stats), revision: revision + 1 })
        }
      }),
    ])
  },
  hydrate: (enabled, stats) => {
    set(state => ({ enabled, statsByTrackId: statsToMap(stats), revision: state.revision + 1, settingsRevision: state.settingsRevision + 1 }))
  },

  setEnabled: enabled => set(state => ({ enabled, settingsRevision: state.settingsRevision + 1 })),

  upsert: (stats) => {
    set((state) => {
      const next = new Map(state.statsByTrackId)
      next.set(stats.track_id, stats)
      return { statsByTrackId: next, revision: state.revision + 1 }
    })
  },

  clear: () => set(state => ({
    statsByTrackId: new Map(),
    clearEpoch: state.clearEpoch + 1,
    revision: state.revision + 1,
  })),
}))
