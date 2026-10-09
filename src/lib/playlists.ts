import { z } from 'zod'
import type { SmartPlaylistSort } from '@/lib/listen-stats'
import type { PlaylistsBundle, Track, TrackListenStats } from '@/types'
import { resolveSmartPlaylistTracks } from '@/lib/listen-stats'

export const ALL_TRACKS_PLAYLIST_ID = 'all' as const
export const LIKED_PLAYLIST_ID = 'liked' as const
export const POPULAR_PLAYLIST_ID = 'popular' as const
export const RECENT_PLAYLIST_ID = 'recent' as const

export type CustomPlaylistId = number
export const BUILTIN_PLAYLIST_IDS = ['all', 'liked', 'popular', 'recent'] as const
export const commonPlaylistIdSchema = z.enum(BUILTIN_PLAYLIST_IDS)
export type CommonPlaylistId = z.infer<typeof commonPlaylistIdSchema>
export type PlaylistId = CustomPlaylistId | CommonPlaylistId

export interface BuiltinPlaylistDefinition {
  name: string
  hideable: boolean
  reorderable: boolean
  downloadable: boolean
  exportable: boolean
  smartSort: SmartPlaylistSort | null
}
export const BUILTIN_PLAYLISTS: Record<CommonPlaylistId, BuiltinPlaylistDefinition> = {
  all: { name: 'All tracks', hideable: false, reorderable: false, downloadable: true, exportable: false, smartSort: null },
  liked: { name: 'Liked', hideable: true, reorderable: true, downloadable: true, exportable: true, smartSort: null },
  popular: { name: 'Popular', hideable: true, reorderable: false, downloadable: false, exportable: false, smartSort: 'likeness' },
  recent: { name: 'Recent', hideable: true, reorderable: false, downloadable: false, exportable: false, smartSort: 'last_played' },
}
export function canReorderPlaylist(id: PlaylistId): boolean {
  return typeof id === 'number' || BUILTIN_PLAYLISTS[id].reorderable
}

export type PlaylistsData = PlaylistsBundle

export type SelectedPlaylist = CustomSelectedPlaylist | CommonSelectedPlaylist

interface BaseSelectedPlaylist {
  id: PlaylistId
  name: string
  trackIds: number[]
  isCustom: boolean
}

export interface CustomSelectedPlaylist extends BaseSelectedPlaylist {
  id: CustomPlaylistId
  isCustom: true
}

export interface CommonSelectedPlaylist extends BaseSelectedPlaylist {
  id: CommonPlaylistId
  isCustom: false
}

export interface ResolvedCustomSelectedPlaylist extends CustomSelectedPlaylist {
  tracks: Track[]
}

export interface ResolvedCommonSelectedPlaylist extends CommonSelectedPlaylist {
  tracks: Track[]
}

export type ResolvedSelectedPlaylist
  = | ResolvedCustomSelectedPlaylist
    | ResolvedCommonSelectedPlaylist

export function isCommonPlaylistId(value: string): value is CommonPlaylistId {
  return commonPlaylistIdSchema.safeParse(value).success
}

function isSmartPlaylistId(
  playlistId: PlaylistId,
): playlistId is typeof POPULAR_PLAYLIST_ID | typeof RECENT_PLAYLIST_ID {
  return (
    playlistId === POPULAR_PLAYLIST_ID || playlistId === RECENT_PLAYLIST_ID
  )
}

function resolvePlaylistTrackIds(
  data: PlaylistsData | null,
  playlistId: PlaylistId,
  libraryTracks: Track[] = [],
  statsByTrackId: ReadonlyMap<number, TrackListenStats> = new Map(),
): number[] {
  if (playlistId === ALL_TRACKS_PLAYLIST_ID) {
    return libraryTracks.map(track => track.id)
  }

  if (isSmartPlaylistId(playlistId)) {
    return resolveSmartPlaylistTracks(
      libraryTracks,
      statsByTrackId,
      BUILTIN_PLAYLISTS[playlistId].smartSort!,
    ).map(track => track.id)
  }

  if (playlistId === LIKED_PLAYLIST_ID) {
    return data?.liked.trackIds ?? []
  }

  const custom = data?.custom.find(playlist => playlist.id === playlistId)
  return custom?.trackIds ?? []
}

export function resolvePlaylistTracks(
  libraryTracks: Track[],
  data: PlaylistsData | null,
  playlistId: PlaylistId,
  statsByTrackId: ReadonlyMap<number, TrackListenStats> = new Map(),
): Track[] {
  if (playlistId === ALL_TRACKS_PLAYLIST_ID) {
    return libraryTracks
  }

  if (isSmartPlaylistId(playlistId)) {
    return resolveSmartPlaylistTracks(
      libraryTracks,
      statsByTrackId,
      BUILTIN_PLAYLISTS[playlistId].smartSort!,
    )
  }

  const trackIds = resolvePlaylistTrackIds(
    data,
    playlistId,
    libraryTracks,
    statsByTrackId,
  )
  if (trackIds.length === 0) {
    return []
  }

  const trackById = new Map(libraryTracks.map(track => [track.id, track]))
  return trackIds
    .map(id => trackById.get(id))
    .filter((track): track is Track => track !== undefined)
}

export function resolveSelectedPlaylist(
  libraryTracks: Track[],
  data: PlaylistsData | null,
  playlistId: PlaylistId,
  statsByTrackId: ReadonlyMap<number, TrackListenStats> = new Map(),
): SelectedPlaylist {
  const trackIds = resolvePlaylistTrackIds(
    data,
    playlistId,
    libraryTracks,
    statsByTrackId,
  )

  if (typeof playlistId === 'string') {
    return { id: playlistId, name: BUILTIN_PLAYLISTS[playlistId].name, trackIds, isCustom: false }
  }

  const custom = data?.custom.find(playlist => playlist.id === playlistId)
  return {
    id: playlistId,
    name: custom?.name ?? 'Playlist',
    trackIds,
    isCustom: true,
  }
}

export function resolveSelectedPlaylistTracks(
  libraryTracks: Track[],
  data: PlaylistsData | null,
  playlistId: PlaylistId,
  statsByTrackId: ReadonlyMap<number, TrackListenStats> = new Map(),
): ResolvedSelectedPlaylist {
  const playlist = resolveSelectedPlaylist(
    libraryTracks,
    data,
    playlistId,
    statsByTrackId,
  )
  const trackById = new Map(libraryTracks.map(track => [track.id, track]))
  const tracks = playlist.trackIds
    .map(id => trackById.get(id))
    .filter((track): track is Track => track !== undefined)

  return { ...playlist, tracks }
}

export function isValidPlaylistId(
  data: PlaylistsData,
  playlistId: PlaylistId,
): boolean {
  if (typeof playlistId === 'string' && isCommonPlaylistId(playlistId)) return true

  return data.custom.some(playlist => playlist.id === playlistId)
}

export function getLikedTrackIdSet(data: PlaylistsData | null): Set<number> {
  if (!data) return new Set()
  return new Set(data.liked.trackIds)
}

export function isTrackLiked(
  data: PlaylistsData | null,
  trackId: number,
): boolean {
  return getLikedTrackIdSet(data).has(trackId)
}

/** Positional selections belong only to this exact ordered membership snapshot. */
export function playlistMembershipKey(playlist: Pick<ResolvedSelectedPlaylist, 'id' | 'trackIds' | 'tracks'>): string {
  return JSON.stringify([playlist.id, playlist.trackIds, playlist.tracks.map(track => track.id)])
}

/** Preserve backend positions when a library refresh temporarily omits a membership. */
export function playlistMembershipIndices(playlist: Pick<ResolvedSelectedPlaylist, 'trackIds' | 'tracks'>): number[] {
  return playlistEntries(playlist).map(entry => entry.sourceIndex)
}

export interface PlaylistEntry { track: Track, sourceIndex: number }

/** Keep each available track paired with its original backend membership slot. */
export function playlistEntries(playlist: Pick<ResolvedSelectedPlaylist, 'trackIds' | 'tracks'>): PlaylistEntry[] {
  const available = new Map(playlist.tracks.map(track => [track.id, track]))
  return playlist.trackIds.flatMap((id, sourceIndex) => {
    const track = available.get(id)
    return track ? [{ track, sourceIndex }] : []
  })
}
