import { create } from 'zustand'
import type { PlaylistsBundle, Track } from '@/lib/db'
import type { TrackListenStats } from '@/types'
import { resolveSmartPlaylistTracks } from '@/stores/listen-stats-store'

export const ALL_TRACKS_PLAYLIST_ID = 'all' as const
export const LIKED_PLAYLIST_ID = 'liked' as const
export const POPULAR_PLAYLIST_ID = 'popular' as const
export const RECENT_PLAYLIST_ID = 'recent' as const

export type CustomPlaylistId = number
export type CommonPlaylistId
  = | typeof ALL_TRACKS_PLAYLIST_ID
    | typeof LIKED_PLAYLIST_ID
    | typeof POPULAR_PLAYLIST_ID
    | typeof RECENT_PLAYLIST_ID
export type PlaylistId = CustomPlaylistId | CommonPlaylistId

const SELECTED_PLAYLIST_STORAGE_KEY = 'soundgrammy:selectedPlaylistId'

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

function isCommonPlaylistId(value: string): value is CommonPlaylistId {
  return (
    value === ALL_TRACKS_PLAYLIST_ID
    || value === LIKED_PLAYLIST_ID
    || value === POPULAR_PLAYLIST_ID
    || value === RECENT_PLAYLIST_ID
  )
}

function readPersistedSelectedPlaylistId(): PlaylistId | number {
  if (typeof window === 'undefined') {
    return ALL_TRACKS_PLAYLIST_ID
  }

  const stored = localStorage.getItem(SELECTED_PLAYLIST_STORAGE_KEY)
  if (!stored || stored === ALL_TRACKS_PLAYLIST_ID) {
    return ALL_TRACKS_PLAYLIST_ID
  }

  if (isCommonPlaylistId(stored)) {
    return stored
  }

  const parsed = Number(stored)
  return Number.isInteger(parsed) && parsed > 0
    ? parsed
    : ALL_TRACKS_PLAYLIST_ID
}

function persistSelectedPlaylistId(id: PlaylistId) {
  if (typeof window === 'undefined') {
    return
  }
  localStorage.setItem(SELECTED_PLAYLIST_STORAGE_KEY, String(id))
}

function normalizePlaylistId(
  data: PlaylistsData,
  playlistId: PlaylistId | number,
): PlaylistId {
  if (
    playlistId === ALL_TRACKS_PLAYLIST_ID
    || playlistId === LIKED_PLAYLIST_ID
    || playlistId === POPULAR_PLAYLIST_ID
    || playlistId === RECENT_PLAYLIST_ID
  ) {
    return playlistId
  }

  if (typeof playlistId === 'number' && playlistId === data.liked.id) {
    return LIKED_PLAYLIST_ID
  }

  return playlistId
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
      playlistId === POPULAR_PLAYLIST_ID ? 'likeness' : 'last_played',
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
      playlistId === POPULAR_PLAYLIST_ID ? 'likeness' : 'last_played',
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

  if (playlistId === ALL_TRACKS_PLAYLIST_ID) {
    return {
      id: ALL_TRACKS_PLAYLIST_ID,
      name: 'All tracks',
      trackIds,
      isCustom: false,
    }
  }

  if (playlistId === LIKED_PLAYLIST_ID) {
    return {
      id: LIKED_PLAYLIST_ID,
      name: 'Liked',
      trackIds,
      isCustom: false,
    }
  }

  if (playlistId === POPULAR_PLAYLIST_ID) {
    return {
      id: POPULAR_PLAYLIST_ID,
      name: 'Most popular',
      trackIds,
      isCustom: false,
    }
  }

  if (playlistId === RECENT_PLAYLIST_ID) {
    return {
      id: RECENT_PLAYLIST_ID,
      name: 'Recent',
      trackIds,
      isCustom: false,
    }
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
  const tracks = resolvePlaylistTracks(
    libraryTracks,
    data,
    playlistId,
    statsByTrackId,
  )

  if (playlist.isCustom) {
    return { ...playlist, tracks }
  }

  return { ...playlist, tracks }
}

function isValidPlaylistId(
  data: PlaylistsData,
  playlistId: PlaylistId,
): boolean {
  if (
    playlistId === ALL_TRACKS_PLAYLIST_ID
    || playlistId === LIKED_PLAYLIST_ID
    || playlistId === POPULAR_PLAYLIST_ID
    || playlistId === RECENT_PLAYLIST_ID
  ) {
    return true
  }

  return data.custom.some(playlist => playlist.id === playlistId)
}

interface PlaylistsState {
  data: PlaylistsData | null
  selectedPlaylistId: PlaylistId
  hydrate: (data: PlaylistsData) => void
  setSelectedPlaylist: (id: PlaylistId) => void
  setData: (data: PlaylistsData) => void
}

export const usePlaylistsStore = create<PlaylistsState>((set, get) => ({
  data: null,
  selectedPlaylistId: ALL_TRACKS_PLAYLIST_ID,

  hydrate: (data) => {
    const persisted = normalizePlaylistId(
      data,
      readPersistedSelectedPlaylistId(),
    )
    const selectedPlaylistId = isValidPlaylistId(data, persisted)
      ? persisted
      : ALL_TRACKS_PLAYLIST_ID

    if (selectedPlaylistId !== persisted) {
      persistSelectedPlaylistId(selectedPlaylistId)
    }

    set({
      data,
      selectedPlaylistId,
    })
  },

  setSelectedPlaylist: (id) => {
    persistSelectedPlaylistId(id)
    set({ selectedPlaylistId: id })
  },

  setData: (data) => {
    const { selectedPlaylistId } = get()
    const nextSelectedPlaylistId = isValidPlaylistId(data, selectedPlaylistId)
      ? selectedPlaylistId
      : ALL_TRACKS_PLAYLIST_ID

    if (nextSelectedPlaylistId !== selectedPlaylistId) {
      persistSelectedPlaylistId(nextSelectedPlaylistId)
    }

    set({
      data,
      selectedPlaylistId: nextSelectedPlaylistId,
    })
  },
}))

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
