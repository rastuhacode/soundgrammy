import {
  BUILTIN_PLAYLIST_IDS,
  BUILTIN_PLAYLISTS,
  isCommonPlaylistId,
  type CommonPlaylistId,
  type PlaylistId,
} from '@/lib/playlists'

const HIDDEN_PLAYLISTS_KEY = 'soundgrammy:hiddenPlaylists'

/** System playlists that may be hidden (not All tracks, not custom). */
export type HideablePlaylistId = Exclude<CommonPlaylistId, 'all'>
export const HIDEABLE_PLAYLIST_IDS = BUILTIN_PLAYLIST_IDS.filter(
  (id): id is HideablePlaylistId => BUILTIN_PLAYLISTS[id].hideable,
)

export function canHidePlaylist(id: PlaylistId): id is HideablePlaylistId {
  return (HIDEABLE_PLAYLIST_IDS as readonly PlaylistId[]).includes(id)
}

function parsePlaylistId(value: unknown): HideablePlaylistId | null {
  if (typeof value === 'string' && isCommonPlaylistId(value) && canHidePlaylist(value)) return value
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

export const HIDEABLE_PLAYLIST_LABELS = Object.fromEntries(
  HIDEABLE_PLAYLIST_IDS.map(id => [id, BUILTIN_PLAYLISTS[id].name]),
) as Record<HideablePlaylistId, string>
