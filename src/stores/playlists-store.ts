import { errorMessage } from '@/lib/errors'
import { isSessionCurrent, createSessionQueue, assertSession } from '@/stores/session-store'
import { create } from 'zustand'
import { api } from '@/lib/api'
import type { CustomPlaylistSummary, PlaylistImportResult } from '@/types'
import { ALL_TRACKS_PLAYLIST_ID, LIKED_PLAYLIST_ID, isCommonPlaylistId, isValidPlaylistId, type PlaylistsData, type PlaylistId } from '@/lib/playlists'

export * from '@/lib/playlists'
const SELECTED_PLAYLIST_STORAGE_KEY = 'soundgrammy:selectedPlaylistId'

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
  if (typeof playlistId === 'string' && isCommonPlaylistId(playlistId)) return playlistId

  if (typeof playlistId === 'number' && playlistId === data.liked.id) {
    return LIKED_PLAYLIST_ID
  }

  return playlistId
}

const enqueue = createSessionQueue()

function requireData(): PlaylistsData {
  const data = usePlaylistsStore.getState().data
  if (!data) throw new Error('Playlists are not loaded. Please try again.')
  return data
}

function playlistTrackIds(id: PlaylistId): number[] {
  const data = requireData()
  if (id === LIKED_PLAYLIST_ID) return data.liked.trackIds
  const playlist = data.custom.find(item => item.id === id)
  if (!playlist) throw new Error('Playlist not found')
  return playlist.trackIds
}

function assertOrder(id: PlaylistId, expected: number[]) {
  const current = playlistTrackIds(id)
  if (current.length !== expected.length || current.some((trackId, index) => trackId !== expected[index])) {
    throw new Error('The playlist changed. Select its tracks again.')
  }
}

function updateCustom(playlist: CustomPlaylistSummary) {
  const data = requireData()
  usePlaylistsStore.getState().setData({
    ...data,
    custom: data.custom.some(item => item.id === playlist.id)
      ? data.custom.map(item => item.id === playlist.id ? playlist : item)
      : [...data.custom, playlist],
  })
}

export interface BulkLikeResult { failedTrackIds: number[] }
interface PlaylistsState {
  data: PlaylistsData | null
  error: string | null
  selectedPlaylistId: PlaylistId
  hydrate: (data: PlaylistsData) => void
  setSelectedPlaylist: (id: PlaylistId) => void
  setData: (data: PlaylistsData) => void
  reset: () => void
  refresh: (firstLoad?: boolean) => Promise<void>
  createPlaylist: (name: string, trackIds?: number[]) => Promise<CustomPlaylistSummary>
  updatePlaylist: (id: number, name: string) => Promise<void>
  deletePlaylist: (id: number) => Promise<void>
  importPlaylist: (path: string, name: string) => Promise<PlaylistImportResult>
  toggleLike: (trackId: number) => Promise<void>
  setLiked: (trackIds: number[], liked: boolean) => Promise<BulkLikeResult>
  addTracks: (id: number, trackIds: number[]) => Promise<void>
  removeTracks: (id: number, positions: number[], expectedTrackIds: number[]) => Promise<void>
  reorderTracks: (id: PlaylistId, trackIds: number[], expectedTrackIds: number[]) => Promise<void>
}

export const usePlaylistsStore = create<PlaylistsState>((set, get) => ({

  reset: () => set({ data: null, error: null, selectedPlaylistId: ALL_TRACKS_PLAYLIST_ID }),
  refresh: (firstLoad = false) => enqueue(async (generation) => {
    try {
      const data = await api.listPlaylists()
      assertSession(generation)
      if (firstLoad) get().hydrate(data)
      else get().setData(data)
      set({ error: null })
    }
    catch (error) {
      if (isSessionCurrent(generation)) set({ error: errorMessage(error) })
      throw error
    }
  }),
  createPlaylist: (name, trackIds = []) => enqueue(async (generation) => {
    requireData()
    const created = await api.createPlaylist({ name, trackIds })
    assertSession(generation)
    updateCustom(created)
    return created
  }),
  updatePlaylist: (id, name) => enqueue(async (generation) => {
    requireData()
    const updated = await api.updatePlaylist({ playlistId: id, name })
    assertSession(generation)
    updateCustom(updated)
  }),
  deletePlaylist: id => enqueue(async (generation) => {
    requireData()
    await api.deletePlaylist(id)
    assertSession(generation)
    const data = requireData()
    get().setData({ ...data, custom: data.custom.filter(item => item.id !== id) })
  }),
  importPlaylist: (path, name) => enqueue(async (generation) => {
    const result = await api.importPlaylistJson(path, name)
    assertSession(generation)
    const data = await api.listPlaylists()
    assertSession(generation)
    get().setData(data)
    return result
  }),
  toggleLike: trackId => enqueue(async (generation) => {
    requireData()
    const liked = await api.toggleLike(trackId)
    assertSession(generation)
    get().setData({ ...requireData(), liked })
  }),
  setLiked: (trackIds, desired) => enqueue(async (generation) => {
    const failedTrackIds: number[] = []
    for (const trackId of new Set(trackIds)) {
      assertSession(generation)
      if (requireData().liked.trackIds.includes(trackId) === desired) continue
      try {
        const liked = await api.toggleLike(trackId)
        assertSession(generation)
        get().setData({ ...requireData(), liked })
      }
      catch {
        assertSession(generation)
        failedTrackIds.push(trackId)
      }
    }
    return { failedTrackIds }
  }),
  addTracks: (id, trackIds) => enqueue(async (generation) => {
    if (trackIds.length === 0) return
    playlistTrackIds(id)
    const updatedAt = await api.addTracksToPlaylist(id, trackIds)
    assertSession(generation)
    const data = requireData()
    get().setData({ ...data, custom: data.custom.map(item => item.id === id
      ? { ...item, updatedAt, trackIds: [...item.trackIds, ...trackIds] }
      : item) })
  }),
  removeTracks: (id, positions, expectedTrackIds) => enqueue(async (generation) => {
    if (positions.length === 0) return
    assertOrder(id, expectedTrackIds)
    const updatedAt = await api.removeTracksFromPlaylist(id, positions, expectedTrackIds)
    assertSession(generation)
    const removed = new Set(positions)
    const data = requireData()
    get().setData({ ...data, custom: data.custom.map(item => item.id === id
      ? { ...item, updatedAt, trackIds: item.trackIds.filter((_, index) => !removed.has(index)) }
      : item) })
  }),
  reorderTracks: (id, trackIds, expectedTrackIds) => enqueue(async (generation) => {
    assertOrder(id, expectedTrackIds)
    const data = requireData()
    const dbId = id === LIKED_PLAYLIST_ID ? data.liked.id : id
    if (typeof dbId !== 'number') throw new Error('This playlist cannot be reordered')
    const updatedAt = await api.reorderPlaylistTracks(dbId, trackIds)
    assertSession(generation)
    const latest = requireData()
    if (id === LIKED_PLAYLIST_ID) get().setData({ ...latest, liked: { ...latest.liked, trackIds, updatedAt } })
    else get().setData({ ...latest, custom: latest.custom.map(item => item.id === id ? { ...item, trackIds, updatedAt } : item) })
  }),

  error: null,
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
