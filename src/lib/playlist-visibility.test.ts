import { describe, expect, it } from 'vitest'
import {
  LIKED_PLAYLIST_ID,
  POPULAR_PLAYLIST_ID,
  RECENT_PLAYLIST_ID,
} from '@/stores/playlists-store'
import {
  canHidePlaylist,
  HIDEABLE_PLAYLIST_IDS,
  parseHiddenPlaylistsJson,
  serializeHiddenPlaylists,
} from './playlist-visibility'

describe('canHidePlaylist', () => {
  it('allows liked, popular, and recent only', () => {
    expect(canHidePlaylist(LIKED_PLAYLIST_ID)).toBe(true)
    expect(canHidePlaylist(POPULAR_PLAYLIST_ID)).toBe(true)
    expect(canHidePlaylist(RECENT_PLAYLIST_ID)).toBe(true)
    expect(canHidePlaylist('all')).toBe(false)
    expect(canHidePlaylist(1)).toBe(false)
  })

  it('lists exactly the hideable system playlists', () => {
    expect([...HIDEABLE_PLAYLIST_IDS]).toEqual([
      LIKED_PLAYLIST_ID,
      POPULAR_PLAYLIST_ID,
      RECENT_PLAYLIST_ID,
    ])
  })
})

describe('parseHiddenPlaylistsJson / serializeHiddenPlaylists', () => {
  it('round-trips hideable ids', () => {
    const serialized = serializeHiddenPlaylists(
      new Set([LIKED_PLAYLIST_ID, POPULAR_PLAYLIST_ID]),
    )
    const read = parseHiddenPlaylistsJson(serialized)
    expect(read.has(LIKED_PLAYLIST_ID)).toBe(true)
    expect(read.has(POPULAR_PLAYLIST_ID)).toBe(true)
    expect(read.has(RECENT_PLAYLIST_ID)).toBe(false)
  })

  it('drops unknown ids and invalid payloads', () => {
    const filtered = parseHiddenPlaylistsJson(
      JSON.stringify(['liked', 'all', 99, 'recent']),
    )
    expect([...filtered].sort()).toEqual(
      [LIKED_PLAYLIST_ID, RECENT_PLAYLIST_ID].sort(),
    )
    expect(parseHiddenPlaylistsJson(null).size).toBe(0)
    expect(parseHiddenPlaylistsJson('not-json').size).toBe(0)
    expect(parseHiddenPlaylistsJson('{}').size).toBe(0)
  })
})
