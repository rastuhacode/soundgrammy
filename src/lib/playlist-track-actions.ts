import type { Track } from '@/lib/db'
import {
  BUILTIN_PLAYLISTS,
  LIKED_PLAYLIST_ID,
  type ResolvedSelectedPlaylist,
  type PlaylistEntry,
} from '@/lib/playlists'

export interface CustomPlaylistRef {
  id: number
  name: string
  trackIds: number[]
}

export type TrackContextActionId
  = | 'select'
    | 'toggleLike'
    | 'addToPlaylist'
    | 'removeFromPlaylist'
    | 'playNext'
    | 'addToEnd'
    | 'cache'
    | 'download'
    | 'removeFromCache'
    | 'showInfo'

export type BulkActionId
  = | 'addToLiked'
    | 'removeFromLiked'
    | 'addToPlaylist'
    | 'removeFromPlaylist'
    | 'playNext'
    | 'addToEnd'
    | 'cache'
    | 'download'

export interface TrackContextActions {
  select: true
  toggleLike: true
  addToPlaylist: true
  removeFromPlaylist: boolean
  playNext: true
  addToEnd: true
  cache: true
  download: true
  removeFromCache: true
  showInfo: true
}

export interface BulkActions {
  addToLiked: boolean
  removeFromLiked: boolean
  addToPlaylist: true
  removeFromPlaylist: boolean
  playNext: true
  addToEnd: true
  cache: true
  download: true
}

/** Non-custom playlists never allow remove-from-playlist. */
export function canRemoveFromPlaylist(
  playlist: Pick<ResolvedSelectedPlaylist, 'isCustom'>,
): boolean {
  return playlist.isCustom
}

/** All tracks, Liked, and custom playlists can be downloaded as a folder + M3U. */
export function canDownloadPlaylist(
  playlist: Pick<ResolvedSelectedPlaylist, 'id' | 'isCustom'>,
): boolean {
  return playlist.isCustom || (typeof playlist.id === 'string' && BUILTIN_PLAYLISTS[playlist.id].downloadable)
}

/** Liked and custom playlists can be exported as a JSON recipe. */
export function canExportPlaylist(
  playlist: Pick<ResolvedSelectedPlaylist, 'id' | 'isCustom'>,
): boolean {
  return playlist.isCustom || (typeof playlist.id === 'string' && BUILTIN_PLAYLISTS[playlist.id].exportable)
}

/**
 * Custom playlists available for "add to playlist".
 * Duplicates are allowed, so every custom playlist is always available.
 */
export function getAvailableCustomPlaylists(
  custom: CustomPlaylistRef[],
): CustomPlaylistRef[] {
  return custom
}

export function getTrackContextActions(
  playlist: Pick<ResolvedSelectedPlaylist, 'isCustom'>,
): TrackContextActions {
  return {
    select: true,
    toggleLike: true,
    addToPlaylist: true,
    removeFromPlaylist: canRemoveFromPlaylist(playlist),
    playNext: true,
    addToEnd: true,
    cache: true,
    download: true,
    removeFromCache: true,
    showInfo: true,
  }
}

export function getBulkActions(
  playlist: Pick<ResolvedSelectedPlaylist, 'id' | 'isCustom'>,
): BulkActions {
  const inLiked = playlist.id === LIKED_PLAYLIST_ID

  return {
    // Outside Liked: add only. Inside Liked: remove only.
    addToLiked: !inLiked,
    removeFromLiked: inLiked,
    addToPlaylist: true,
    removeFromPlaylist: canRemoveFromPlaylist(playlist),
    playNext: true,
    addToEnd: true,
    cache: true,
    download: true,
  }
}

/** One ordered membership model for table rendering and playback. */
export function sortPlaylistEntries(
  entries: PlaylistEntry[],
  sorting: { id: string, desc: boolean }[],
): PlaylistEntry[] {
  const sorts = sorting.filter((sort): sort is TrackSortState =>
    sort.id === 'title' || sort.id === 'performer' || sort.id === 'duration',
  )
  if (!sorts.length) return entries
  return [...entries].sort((a, b) => {
    for (const sort of sorts) {
      const compared = compareTracks(a.track, b.track, sort)
      if (compared !== 0) return compared
    }
    return a.sourceIndex - b.sourceIndex
  })
}

export type TrackSortColumn = 'title' | 'performer' | 'duration'

export interface TrackSortState {
  id: TrackSortColumn
  desc: boolean
}

function compareNullableString(a: string | null, b: string | null): number {
  if (a === null && b === null) return 0
  if (a === null) return 1
  if (b === null) return -1
  return a.toLocaleLowerCase().localeCompare(b.toLocaleLowerCase())
}

function compareNullableNumber(a: number | null, b: number | null): number {
  if (a === null && b === null) return 0
  if (a === null) return 1
  if (b === null) return -1
  return a - b
}

export function compareTracks(
  a: Track,
  b: Track,
  sort: TrackSortState,
): number {
  let result = 0
  switch (sort.id) {
    case 'title':
      result = compareNullableString(a.title, b.title)
      break
    case 'performer':
      result = compareNullableString(a.performer, b.performer)
      break
    case 'duration':
      result = compareNullableNumber(a.duration, b.duration)
      break
  }
  return sort.desc ? -result : result
}

export function formatTrackDuration(seconds: number | null): string {
  if (seconds === null) return '--:--'
  const mins = Math.floor(seconds / 60)
  const secs = seconds % 60
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

/** Move the item at `fromIndex` to `toIndex` within a list. */
export function reorderByIndex<T>(
  order: T[],
  fromIndex: number,
  toIndex: number,
): T[] {
  if (
    fromIndex < 0
    || toIndex < 0
    || fromIndex >= order.length
    || toIndex >= order.length
    || fromIndex === toIndex
  ) {
    return order
  }
  const next = [...order]
  const [moved] = next.splice(fromIndex, 1)
  next.splice(toIndex, 0, moved!)
  return next
}

/** Stable sortable identities for tracks, including duplicate memberships. */
export function getTrackSortableIds(trackIds: number[]): string[] {
  const occurrences = new Map<number, number>()
  return trackIds.map((trackId) => {
    const occurrence = occurrences.get(trackId) ?? 0
    occurrences.set(trackId, occurrence + 1)
    return `${trackId}:${occurrence}`
  })
}
