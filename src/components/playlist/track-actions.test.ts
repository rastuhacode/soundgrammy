import { describe, expect, it } from 'vitest'
import type { Track } from '@/lib/db'
import {
  canRemoveFromPlaylist,
  compareTracks,
  enterSelectionWithTrack,
  filterAndSortTrackIds,
  getAvailableCustomPlaylists,
  getBulkActions,
  getTrackContextActions,
  reorderTrackIds,
  selectionModeAfterPlaylistChange,
  sortTracks,
  toPlayablePlaylist,
} from './track-actions'

function track(
  id: number,
  overrides: Partial<Track> = {},
): Track {
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

describe('canRemoveFromPlaylist', () => {
  it('allows remove only for custom playlists', () => {
    expect(canRemoveFromPlaylist({ isCustom: true })).toBe(true)
    expect(canRemoveFromPlaylist({ isCustom: false })).toBe(false)
  })
})

describe('getTrackContextActions', () => {
  it('hides removeFromPlaylist for All tracks / Liked', () => {
    const actions = getTrackContextActions({ isCustom: false })
    expect(actions.select).toBe(true)
    expect(actions.toggleLike).toBe(true)
    expect(actions.addToPlaylist).toBe(true)
    expect(actions.removeFromPlaylist).toBe(false)
    expect(actions.download).toBe(true)
    expect(actions.showInfo).toBe(true)
  })

  it('shows removeFromPlaylist for custom playlists', () => {
    expect(getTrackContextActions({ isCustom: true }).removeFromPlaylist).toBe(true)
  })
})

describe('getBulkActions', () => {
  it('mirrors playlist boundary for mass remove', () => {
    expect(
      getBulkActions({ id: 'all', isCustom: false }).removeFromPlaylist,
    ).toBe(false)
    expect(
      getBulkActions({ id: 1, isCustom: true }).removeFromPlaylist,
    ).toBe(true)
  })

  it('hides remove-from-liked outside the Liked playlist', () => {
    const actions = getBulkActions({ id: 'all', isCustom: false })
    expect(actions.addToLiked).toBe(true)
    expect(actions.removeFromLiked).toBe(false)
    expect(actions.addToPlaylist).toBe(true)
    expect(actions.download).toBe(true)
  })

  it('shows only remove-from-liked when viewing Liked', () => {
    const actions = getBulkActions({ id: 'liked', isCustom: false })
    expect(actions.addToLiked).toBe(false)
    expect(actions.removeFromLiked).toBe(true)
  })
})

describe('getAvailableCustomPlaylists', () => {
  const playlists = [
    { id: 1, name: 'A', trackIds: [10, 20] },
    { id: 2, name: 'B', trackIds: [10] },
    { id: 3, name: 'C', trackIds: [] },
  ]

  it('excludes playlists that already contain the single track', () => {
    expect(getAvailableCustomPlaylists(playlists, [10]).map(p => p.id)).toEqual([
      3,
    ])
  })

  it('keeps playlists missing at least one of the selected tracks', () => {
    expect(getAvailableCustomPlaylists(playlists, [10, 20]).map(p => p.id)).toEqual([
      2,
      3,
    ])
  })

  it('returns all playlists when selection is empty', () => {
    expect(getAvailableCustomPlaylists(playlists, []).map(p => p.id)).toEqual([
      1,
      2,
      3,
    ])
  })
})

describe('filterAndSortTrackIds (regression)', () => {
  const tracks = [
    track(1, { title: 'Banana', performer: 'Zed', duration: 30 }),
    track(2, { title: 'Apple', performer: 'Ann', duration: 10 }),
    track(3, { title: 'Cherry', performer: 'Bob', duration: 20 }),
  ]

  const contains = (haystack: string, needle: string) =>
    needle.trim() === ''
    || haystack.toLowerCase().includes(needle.toLowerCase())

  it('filters by performer - title then sorts by title asc', () => {
    expect(
      filterAndSortTrackIds(tracks, '', { id: 'title', desc: false }, contains),
    ).toEqual([2, 1, 3])
  })

  it('sorts by duration desc', () => {
    expect(
      filterAndSortTrackIds(tracks, '', { id: 'duration', desc: true }, contains),
    ).toEqual([1, 3, 2])
  })

  it('applies search before sort', () => {
    expect(
      filterAndSortTrackIds(tracks, 'ann', { id: 'title', desc: false }, contains),
    ).toEqual([2])
  })

  it('preserves input order when sort is null', () => {
    expect(filterAndSortTrackIds(tracks, '', null, contains)).toEqual([1, 2, 3])
  })
})

describe('toPlayablePlaylist', () => {
  it('reorders trackIds to match the visible sorted tracks', () => {
    const playlist = {
      id: 'all' as const,
      name: 'All tracks',
      isCustom: false as const,
      trackIds: [1, 2, 3],
      tracks: [track(1), track(2), track(3)],
    }
    const ordered = [track(3), track(1)]
    const playable = toPlayablePlaylist(playlist, ordered)
    expect(playable.trackIds).toEqual([3, 1])
    expect(playable.tracks.map(t => t.id)).toEqual([3, 1])
  })
})

describe('sortTracks', () => {
  it('returns the same reference when sort is null', () => {
    const tracks = [track(1), track(2)]
    expect(sortTracks(tracks, null)).toBe(tracks)
  })
})

describe('compareTracks', () => {
  it('orders null titles after named titles ascending', () => {
    const named = track(1, { title: 'A' })
    const missing = track(2, { title: null })
    expect(compareTracks(named, missing, { id: 'title', desc: false })).toBeLessThan(0)
  })
})

describe('selection mode helpers', () => {
  it('enters selection with the chosen track', () => {
    expect(enterSelectionWithTrack(42)).toEqual({
      selectionMode: true,
      rowSelection: { 42: true },
    })
  })

  it('clears selection when playlist changes', () => {
    expect(selectionModeAfterPlaylistChange()).toEqual({
      selectionMode: false,
      rowSelection: {},
    })
  })
})

describe('reorderTrackIds', () => {
  it('moves active id to over id index', () => {
    expect(reorderTrackIds([1, 2, 3, 4], 1, 3)).toEqual([2, 3, 1, 4])
  })

  it('returns the same array when ids are missing or unchanged', () => {
    const order = [1, 2, 3]
    expect(reorderTrackIds(order, 1, 1)).toBe(order)
    expect(reorderTrackIds(order, 9, 2)).toBe(order)
  })
})
