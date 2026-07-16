import type { Track } from '@/lib/db'
import {
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
    | 'download'
    | 'showInfo'

export type BulkActionId
  = | 'addToLiked'
    | 'removeFromLiked'
    | 'addToPlaylist'
    | 'removeFromPlaylist'
    | 'download'

export interface TrackContextActions {
  select: true
  toggleLike: true
  addToPlaylist: true
  removeFromPlaylist: boolean
  download: true
  showInfo: true
}

export interface BulkActions {
  addToLiked: boolean
  removeFromLiked: boolean
  addToPlaylist: true
  removeFromPlaylist: boolean
  download: true
}

/** All tracks and Liked are non-custom; only custom playlists allow remove-from-playlist. */
export function canRemoveFromPlaylist(
  playlist: Pick<ResolvedSelectedPlaylist, 'isCustom'>,
): boolean {
  return playlist.isCustom
}

/**
 * Custom playlists that do not already contain every selected track.
 * For a single track this matches the previous dropdown filter.
 */
export function getAvailableCustomPlaylists(
  custom: CustomPlaylistRef[],
  trackIds: number[],
): CustomPlaylistRef[] {
  if (trackIds.length === 0) return custom

  return custom.filter(playlist =>
    trackIds.some(id => !playlist.trackIds.includes(id)),
  )
}

export function getTrackContextActions(
  playlist: Pick<ResolvedSelectedPlaylist, 'isCustom'>,
): TrackContextActions {
  return {
    select: true,
    toggleLike: true,
    addToPlaylist: true,
    removeFromPlaylist: canRemoveFromPlaylist(playlist),
    download: true,
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
    download: true,
  }
}

/** Build a playable playlist snapshot that follows the current visible (filtered/sorted) order. */
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

export function sortTracks(
  tracks: Track[],
  sort: TrackSortState | null,
): Track[] {
  if (!sort) return tracks
  return [...tracks].sort((a, b) => compareTracks(a, b, sort))
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

export function enterSelectionWithTrack(trackId: number): {
  selectionMode: true
  rowSelection: Record<string, boolean>
} {
  return {
    selectionMode: true,
    rowSelection: { [String(trackId)]: true },
  }
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
  const next = [...order]
  const [moved] = next.splice(oldIndex, 1)
  next.splice(newIndex, 0, moved!)
  return next
}
