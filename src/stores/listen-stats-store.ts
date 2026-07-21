import { create } from 'zustand'
import type { Track } from '@/lib/db'
import type { TrackListenStats } from '@/types'

export type SmartPlaylistSort = 'likeness' | 'last_played'

function statsToMap(
  stats: TrackListenStats[],
): Map<number, TrackListenStats> {
  const next = new Map<number, TrackListenStats>()
  for (const row of stats) {
    next.set(row.track_id, row)
  }
  return next
}

/** Library tracks that have listen history, sorted for a smart playlist. */
export function resolveSmartPlaylistTracks(
  libraryTracks: Track[],
  statsByTrackId: ReadonlyMap<number, TrackListenStats>,
  sort: SmartPlaylistSort,
): Track[] {
  const withStats: Array<{ track: Track, stats: TrackListenStats }> = []
  for (const track of libraryTracks) {
    const stats = statsByTrackId.get(track.id)
    if (stats) {
      withStats.push({ track, stats })
    }
  }

  if (sort === 'likeness') {
    withStats.sort((a, b) => {
      const likenessDiff = b.stats.likeness - a.stats.likeness
      if (likenessDiff !== 0) return likenessDiff
      return a.track.id - b.track.id
    })
  }
  else {
    withStats.sort((a, b) => {
      const aLast = a.stats.last_played_at_ms
      const bLast = b.stats.last_played_at_ms
      if (aLast == null && bLast == null) {
        return a.track.id - b.track.id
      }
      if (aLast == null) return 1
      if (bLast == null) return -1
      if (bLast !== aLast) return bLast - aLast
      return a.track.id - b.track.id
    })
  }

  return withStats.map(entry => entry.track)
}

export function smartPlaylistTrackCount(
  libraryTracks: Track[],
  statsByTrackId: ReadonlyMap<number, TrackListenStats>,
): number {
  let count = 0
  for (const track of libraryTracks) {
    if (statsByTrackId.has(track.id)) count += 1
  }
  return count
}

/** Newest last_played_at_ms among library tracks that have stats (for Recency sort). */
export function smartPlaylistUpdatedAt(
  libraryTracks: Track[],
  statsByTrackId: ReadonlyMap<number, TrackListenStats>,
): string {
  let newest: number | null = null
  for (const track of libraryTracks) {
    const last = statsByTrackId.get(track.id)?.last_played_at_ms
    if (last == null) continue
    if (newest == null || last > newest) newest = last
  }
  return newest == null ? '' : String(newest)
}

interface ListenStatsState {
  statsByTrackId: Map<number, TrackListenStats>
  hydrate: (stats: TrackListenStats[]) => void
  upsert: (stats: TrackListenStats) => void
}

export const useListenStatsStore = create<ListenStatsState>(set => ({
  statsByTrackId: new Map(),

  hydrate: (stats) => {
    set({ statsByTrackId: statsToMap(stats) })
  },

  upsert: (stats) => {
    set((state) => {
      const next = new Map(state.statsByTrackId)
      next.set(stats.track_id, stats)
      return { statsByTrackId: next }
    })
  },
}))
