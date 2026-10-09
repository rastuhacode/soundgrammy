import { describe, expect, it } from 'vitest'
import type { Track } from '@/lib/db'
import {
  canDownloadPlaylist,
  canExportPlaylist,
  canRemoveFromPlaylist,
  compareTracks,
  sortPlaylistEntries,
  getAvailableCustomPlaylists,
  getBulkActions,
  getTrackContextActions,
  getTrackSortableIds,
  reorderByIndex,
} from '@/lib/playlist-track-actions'

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

describe('canDownloadPlaylist', () => {
  it('allows All tracks, Liked, and custom', () => {
    expect(canDownloadPlaylist({ id: 'all', isCustom: false })).toBe(true)
    expect(canDownloadPlaylist({ id: 'liked', isCustom: false })).toBe(true)
    expect(canDownloadPlaylist({ id: 3, isCustom: true })).toBe(true)
  })

  it('blocks Popular and Recent', () => {
    expect(canDownloadPlaylist({ id: 'popular', isCustom: false })).toBe(false)
    expect(canDownloadPlaylist({ id: 'recent', isCustom: false })).toBe(false)
  })
})

describe('canExportPlaylist', () => {
  it('allows Liked and custom only', () => {
    expect(canExportPlaylist({ id: 'liked', isCustom: false })).toBe(true)
    expect(canExportPlaylist({ id: 3, isCustom: true })).toBe(true)
  })

  it('blocks All tracks, Popular, and Recent', () => {
    expect(canExportPlaylist({ id: 'all', isCustom: false })).toBe(false)
    expect(canExportPlaylist({ id: 'popular', isCustom: false })).toBe(false)
    expect(canExportPlaylist({ id: 'recent', isCustom: false })).toBe(false)
  })
})

describe('getTrackContextActions', () => {
  it('hides removeFromPlaylist for All tracks / Liked', () => {
    const actions = getTrackContextActions({ isCustom: false })
    expect(actions.select).toBe(true)
    expect(actions.toggleLike).toBe(true)
    expect(actions.addToPlaylist).toBe(true)
    expect(actions.removeFromPlaylist).toBe(false)
    expect(actions.cache).toBe(true)
    expect(actions.download).toBe(true)
    expect(actions.removeFromCache).toBe(true)
    expect(actions.showInfo).toBe(true)
  })

  it('shows removeFromPlaylist for custom playlists', () => {
    expect(getTrackContextActions({ isCustom: true }).removeFromPlaylist).toBe(true)
  })

  it('includes playNext and addToEnd', () => {
    expect(getTrackContextActions({ isCustom: false }).playNext).toBe(true)
    expect(getTrackContextActions({ isCustom: false }).addToEnd).toBe(true)
    expect(getBulkActions({ id: 'all', isCustom: false }).playNext).toBe(true)
    expect(getBulkActions({ id: 'all', isCustom: false }).addToEnd).toBe(true)
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
    expect(actions.cache).toBe(true)
    expect(actions.download).toBe(true)
  })

  it('shows only remove-from-liked when viewing Liked', () => {
    const actions = getBulkActions({ id: 'liked', isCustom: false })
    expect(actions.addToLiked).toBe(false)
    expect(actions.removeFromLiked).toBe(true)
  })

  it('hides removeFromPlaylist for popular and recent', () => {
    expect(
      getBulkActions({ id: 'popular', isCustom: false }).removeFromPlaylist,
    ).toBe(false)
    expect(
      getBulkActions({ id: 'recent', isCustom: false }).removeFromPlaylist,
    ).toBe(false)
    expect(
      getBulkActions({ id: 'popular', isCustom: false }).addToLiked,
    ).toBe(true)
  })

  it('exposes cache and download for every playlist view', () => {
    for (const id of ['all', 'liked', 'popular', 'recent', 1] as const) {
      const actions = getBulkActions({
        id,
        isCustom: typeof id === 'number',
      })
      expect(actions.cache).toBe(true)
      expect(actions.download).toBe(true)
    }
  })
})

describe('getAvailableCustomPlaylists', () => {
  const playlists = [
    { id: 1, name: 'A', trackIds: [10, 20] },
    { id: 2, name: 'B', trackIds: [10] },
    { id: 3, name: 'C', trackIds: [] },
  ]

  it('returns all custom playlists even when they already contain the track', () => {
    expect(getAvailableCustomPlaylists(playlists).map(p => p.id)).toEqual([
      1,
      2,
      3,
    ])
  })

  it('returns all playlists for a multi-track selection', () => {
    expect(getAvailableCustomPlaylists(playlists).map(p => p.id)).toEqual([
      1,
      2,
      3,
    ])
  })

  it('returns all playlists when selection is empty', () => {
    expect(getAvailableCustomPlaylists(playlists).map(p => p.id)).toEqual([
      1,
      2,
      3,
    ])
  })
})

describe('sortPlaylistEntries', () => {
  it('uses every sort column and preserves duplicate membership indexes on ties', () => {
    const entries = [
      { track: track(1, { title: 'Same', performer: 'Zed' }), sourceIndex: 0 },
      { track: track(2, { title: 'Same', performer: 'Ann' }), sourceIndex: 1 },
      { track: track(1, { title: 'Same', performer: 'Zed' }), sourceIndex: 2 },
      { track: track(3, { title: null }), sourceIndex: 3 },
    ]
    const result = sortPlaylistEntries(entries, [{ id: 'title', desc: false }, { id: 'performer', desc: false }])
    expect(result.map(entry => entry.sourceIndex)).toEqual([1, 0, 2, 3])
    expect(entries.map(entry => entry.sourceIndex)).toEqual([0, 1, 2, 3])
    expect(sortPlaylistEntries(entries, [])).toBe(entries)
  })
})

describe('compareTracks', () => {
  it('orders null titles after named titles ascending', () => {
    const named = track(1, { title: 'A' })
    const missing = track(2, { title: null })
    expect(compareTracks(named, missing, { id: 'title', desc: false })).toBeLessThan(0)
  })
})

describe('reorderByIndex', () => {
  it('moves an item by index', () => {
    expect(reorderByIndex([2, 1, 1], 2, 1)).toEqual([2, 1, 1])
    expect(reorderByIndex(['b', 'a1', 'a2'], 2, 1)).toEqual(['b', 'a2', 'a1'])
  })

  it('returns the same reference when indexes are unchanged', () => {
    const order = [1, 2, 3]
    expect(reorderByIndex(order, 1, 1)).toBe(order)
  })
})

describe('getTrackSortableIds', () => {
  it('keeps duplicate track memberships uniquely identifiable', () => {
    expect(getTrackSortableIds([2, 1, 2, 2])).toEqual([
      '2:0',
      '1:0',
      '2:1',
      '2:2',
    ])
  })
})
