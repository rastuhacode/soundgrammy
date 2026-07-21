import {
  LIKED_PLAYLIST_ID,
  POPULAR_PLAYLIST_ID,
  RECENT_PLAYLIST_ID,
  type PlaylistId,
} from '@/stores/playlists-store'

const HIDDEN_PLAYLISTS_KEY = 'soundgrammy:hiddenPlaylists'

/** System playlists that may be hidden (not All tracks, not custom). */
export const HIDEABLE_PLAYLIST_IDS = [
  LIKED_PLAYLIST_ID,
  POPULAR_PLAYLIST_ID,
  RECENT_PLAYLIST_ID,
] as const

export type HideablePlaylistId = (typeof HIDEABLE_PLAYLIST_IDS)[number]

export function canHidePlaylist(id: PlaylistId): id is HideablePlaylistId {
  return (HIDEABLE_PLAYLIST_IDS as readonly PlaylistId[]).includes(id)
}

function parsePlaylistId(value: unknown): HideablePlaylistId | null {
  if (
    value === LIKED_PLAYLIST_ID
    || value === POPULAR_PLAYLIST_ID
    || value === RECENT_PLAYLIST_ID
  ) {
    return value
  }
  return null
}

/** Parse stored JSON into a hideable-id set (drops unknowns). */
export function parseHiddenPlaylistsJson(stored: string | null): Set<HideablePlaylistId> {
  if (!stored) return new Set()
  try {
    const parsed = JSON.parse(stored) as unknown
    if (!Array.isArray(parsed)) return new Set()
    const ids = parsed
      .map(parsePlaylistId)
      .filter((id): id is HideablePlaylistId => id !== null)
    return new Set(ids)
  }
  catch {
    return new Set()
  }
}

export function serializeHiddenPlaylists(hidden: Set<HideablePlaylistId>): string {
  return JSON.stringify([...hidden])
}

export function readHiddenPlaylists(): Set<HideablePlaylistId> {
  if (typeof window === 'undefined') return new Set()
  return parseHiddenPlaylistsJson(localStorage.getItem(HIDDEN_PLAYLISTS_KEY))
}

export function writeHiddenPlaylists(hidden: Set<HideablePlaylistId>) {
  if (typeof window === 'undefined') return
  localStorage.setItem(HIDDEN_PLAYLISTS_KEY, serializeHiddenPlaylists(hidden))
}

export const HIDEABLE_PLAYLIST_LABELS: Record<HideablePlaylistId, string> = {
  [LIKED_PLAYLIST_ID]: 'Liked',
  [POPULAR_PLAYLIST_ID]: 'Popular',
  [RECENT_PLAYLIST_ID]: 'Recent',
}
