import type { Track } from '@/lib/db'
import {
  ALL_TRACKS_PLAYLIST_ID,
  LIKED_PLAYLIST_ID,
  type ResolvedSelectedPlaylist,
} from '@/stores/playlists-store'

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
  if (playlist.isCustom) return true
  return (
    playlist.id === ALL_TRACKS_PLAYLIST_ID
    || playlist.id === LIKED_PLAYLIST_ID
  )
}

/** Liked and custom playlists can be exported as a JSON recipe. */
export function canExportPlaylist(
  playlist: Pick<ResolvedSelectedPlaylist, 'id' | 'isCustom'>,
): boolean {
  if (playlist.isCustom) return true
  return playlist.id === LIKED_PLAYLIST_ID
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

/** Build a playable playlist snapshot from an explicit track order. */
export function toPlayablePlaylist(
  playlist: ResolvedSelectedPlaylist,
  orderedTracks: Track[],
): ResolvedSelectedPlaylist {
  const trackIds = orderedTracks.map(track => track.id)

  if (playlist.isCustom) {
    return {
      ...playlist,
      trackIds,
      tracks: orderedTracks,
    }
  }

  return {
    ...playlist,
    trackIds,
    tracks: orderedTracks,
  }
}

/** Membership-aware sort so duplicate track ids keep a stable slot order. */
export function sortIndexedPlaylistTracks(
  tracks: Track[],
  sort: TrackSortState | null,
): { track: Track, sourceIndex: number }[] {
  const indexed = tracks.map((track, sourceIndex) => ({ track, sourceIndex }))
  if (!sort) return indexed
  return [...indexed].sort((a, b) => {
    const cmp = compareTracks(a.track, b.track, sort)
    return cmp !== 0 ? cmp : a.sourceIndex - b.sourceIndex
  })
}

export function sortTracks(
  tracks: Track[],
  sort: TrackSortState | null,
): Track[] {
  if (!sort) return tracks
  return sortIndexedPlaylistTracks(tracks, sort).map(({ track }) => track)
}

export function sortingStateToTrackSort(
  sorting: { id: string, desc: boolean }[],
): TrackSortState | null {
  const first = sorting[0]
  if (!first) return null
  if (
    first.id !== 'title'
    && first.id !== 'performer'
    && first.id !== 'duration'
  ) {
    return null
  }
  return { id: first.id, desc: first.desc }
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

/** Search filter → sort → ordered track ids (regression surface for the table pipeline). */
export function filterAndSortTrackIds(
  tracks: Track[],
  search: string,
  sort: TrackSortState | null,
  contains: (haystack: string, needle: string) => boolean,
): number[] {
  const filtered = tracks.filter(track =>
    contains(`${track.performer} - ${track.title}`, search),
  )

  if (!sort) {
    return filtered.map(track => track.id)
  }

  return [...filtered]
    .sort((a, b) => compareTracks(a, b, sort))
    .map(track => track.id)
}

export function formatTrackDuration(seconds: number | null): string {
  if (seconds === null) return '--:--'
  const mins = Math.floor(seconds / 60)
  const secs = seconds % 60
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

export function selectionModeAfterPlaylistChange(): {
  selectionMode: false
  rowSelection: Record<string, boolean>
} {
  return { selectionMode: false, rowSelection: {} }
}

export function enterSelectionWithTrack(rowId: number): {
  selectionMode: true
  rowSelection: Record<string, boolean>
} {
  return {
    selectionMode: true,
    rowSelection: { [String(rowId)]: true },
  }
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

/** Move `activeId` to the index of `overId` within a track id list. */
export function reorderTrackIds(
  order: number[],
  activeId: number,
  overId: number,
): number[] {
  const oldIndex = order.indexOf(activeId)
  const newIndex = order.indexOf(overId)
  if (oldIndex < 0 || newIndex < 0 || oldIndex === newIndex) {
    return order
  }
  return reorderByIndex(order, oldIndex, newIndex)
}
