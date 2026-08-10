import { describe, expect, it } from 'vitest'
import type { Track } from '@/lib/db'
import type { TrackListenStats } from '@/types'
import {
  resolveSmartPlaylistTracks,
  smartPlaylistTrackCount,
  smartPlaylistUpdatedAt,
  useListenStatsStore,
} from './listen-stats-store'

function track(id: number, overrides: Partial<Track> = {}): Track {
  return {
    id,
    tg_user_id: 1,
    file_id: `f${id}`,
    file_unique_id: `u${id}`,
    title: `Title ${id}`,
    performer: `Artist ${id}`,
    duration: id * 10,
    source: 'telegram',
    mime_type: 'audio/mpeg',
    file_size: 1000,
    created_at: '2024-01-01T00:00:00Z',
    ...overrides,
  }
}

function stats(
  trackId: number,
  overrides: Partial<TrackListenStats> = {},
): TrackListenStats {
  return {
    track_id: trackId,
    starts: 1,
    qualified_plays: 0,
    completes: 0,
    early_skips: 0,
    total_listened_ms: 0,
    first_played_at_ms: null,
    last_played_at_ms: null,
    likeness: 0,
    ...overrides,
  }
}

describe('resolveSmartPlaylistTracks', () => {
  const library = [track(1), track(2), track(3), track(4)]

  it('returns empty when no stats', () => {
    expect(resolveSmartPlaylistTracks(library, new Map(), 'likeness')).toEqual(
      [],
    )
  })

  it('drops stats for tracks missing from the library', () => {
    const byId = new Map([
      [2, stats(2, { likeness: 5 })],
      [99, stats(99, { likeness: 100 })],
    ])
    const result = resolveSmartPlaylistTracks(library, byId, 'likeness')
    expect(result.map(t => t.id)).toEqual([2])
  })

  it('orders by likeness descending with stable track_id tie-break', () => {
    const byId = new Map([
      [1, stats(1, { likeness: 2 })],
      [2, stats(2, { likeness: 5 })],
      [3, stats(3, { likeness: 5 })],
      [4, stats(4, { likeness: 1 })],
    ])
    const result = resolveSmartPlaylistTracks(library, byId, 'likeness')
    expect(result.map(t => t.id)).toEqual([2, 3, 1, 4])
  })

  it('orders by last_played_at_ms descending with nulls last', () => {
    const byId = new Map([
      [1, stats(1, { last_played_at_ms: 100 })],
      [2, stats(2, { last_played_at_ms: 300 })],
      [3, stats(3, { last_played_at_ms: null })],
      [4, stats(4, { last_played_at_ms: 200 })],
    ])
    const result = resolveSmartPlaylistTracks(library, byId, 'last_played')
    expect(result.map(t => t.id)).toEqual([2, 4, 1, 3])
  })
})

describe('smartPlaylistTrackCount / smartPlaylistUpdatedAt', () => {
  it('counts only library ∩ stats', () => {
    const library = [track(1), track(2)]
    const byId = new Map([
      [1, stats(1)],
      [99, stats(99)],
    ])
    expect(smartPlaylistTrackCount(library, byId)).toBe(1)
  })

  it('uses newest last_played_at_ms for updatedAt', () => {
    const library = [track(1), track(2)]
    const byId = new Map([
      [1, stats(1, { last_played_at_ms: 50 })],
      [2, stats(2, { last_played_at_ms: 90 })],
    ])
    expect(smartPlaylistUpdatedAt(library, byId)).toBe('90')
  })

  it('returns empty updatedAt when nothing has last_played', () => {
    const library = [track(1)]
    const byId = new Map([[1, stats(1)]])
    expect(smartPlaylistUpdatedAt(library, byId)).toBe('')
  })
})

describe('listen statistics preferences', () => {
  it('hydrates the enabled setting and clears all smart-playlist data', () => {
    useListenStatsStore.getState().hydrate(false, [stats(1)])
    const beforeClearEpoch = useListenStatsStore.getState().clearEpoch
    expect(useListenStatsStore.getState().enabled).toBe(false)
    expect(useListenStatsStore.getState().statsByTrackId.has(1)).toBe(true)

    useListenStatsStore.getState().clear()
    expect(useListenStatsStore.getState().statsByTrackId.size).toBe(0)
    expect(useListenStatsStore.getState().clearEpoch).toBe(beforeClearEpoch + 1)
  })
})
