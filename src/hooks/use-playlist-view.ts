import type { Track } from '@/lib/db'
import { useLibraryStore } from '@/stores/library-store'
import { usePlayerStore } from '@/stores/player-store'
import {
  ALL_TRACKS_PLAYLIST_ID,
  getLikedTrackIdSet,
  isTrackLiked,
  LIKED_PLAYLIST_ID,
  POPULAR_PLAYLIST_ID,
  RECENT_PLAYLIST_ID,
  resolveSelectedPlaylistTracks,
  usePlaylistsStore,
} from '@/stores/playlists-store'
import type { CustomPlaylistId } from '@/stores/playlists-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { useMemo, useState } from 'react'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import type { RowSelectionState, SortingState } from '@tanstack/react-table'
import { api } from '@/lib/api'
import { resolvePlayingSourceIndex } from '@/lib/queue/playing-source-index'
import { useFilter } from '@/hooks/utils/use-filter'
import {
  enterSelectionWithTrack,
  sortIndexedPlaylistTracks,
  sortingStateToTrackSort,
} from '@/components/playlist/track-actions'
import { useCacheStore } from '@/stores/cache-store'

function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string' && message.length > 0) return message
  }
  if (error instanceof Error && error.message) return error.message
  return 'Something went wrong'
}

export function usePlaylistView() {
  const [search, setSearch] = useState('')
  const [selectionMode, setSelectionMode] = useState(false)
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({})
  const [sorting, setSorting] = useState<SortingState>([])
  const [infoTrack, setInfoTrack] = useState<Track | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)

  const { contains } = useFilter()

  const libraryTracks = useLibraryStore(state => state.library)
  const statsByTrackId = useListenStatsStore(state => state.statsByTrackId)
  const currentTrackId = usePlayerStore(
    state => state.currentTrack?.id ?? null,
  )
  const queue = usePlayerStore(state => state.queue)
  const isPlaying = usePlayerStore(state => state.isPlaying)
  const playPlaylist = usePlayerStore(state => state.playPlaylist)
  const setPlaying = usePlayerStore(state => state.setPlaying)
  const enqueueNext = usePlayerStore(state => state.enqueueNext)
  const appendToQueue = usePlayerStore(state => state.appendToQueue)
  const data = usePlaylistsStore(state => state.data)
  const selectedPlaylistId = usePlaylistsStore(
    state => state.selectedPlaylistId,
  )
  const setData = usePlaylistsStore(state => state.setData)

  const selectedPlaylist = useMemo(
    () => resolveSelectedPlaylistTracks(
      libraryTracks,
      data,
      selectedPlaylistId,
      statsByTrackId,
    ),
    [libraryTracks, data, selectedPlaylistId, statsByTrackId],
  )
  const {
    tracks: playlistTracks,
    isCustom,
    id: playlistId,
  } = selectedPlaylist

  const playingSourceIndex = useMemo(() => {
    return resolvePlayingSourceIndex({
      currentTrackId,
      playlistId,
      playlistTrackIds: playlistTracks.map(track => track.id),
      queue: {
        cursor: queue.cursor,
        source: queue.source,
        sourceIndices: queue.sourceIndices,
        trackIds: queue.tracks.map(track => track.id),
      },
    })
  }, [
    currentTrackId,
    playlistId,
    playlistTracks,
    queue.cursor,
    queue.source,
    queue.sourceIndices,
    queue.tracks,
  ])

  const filteredIndexedTracks = useMemo(
    () =>
      playlistTracks
        .map((track, sourceIndex) => ({ track, sourceIndex }))
        .filter(({ track }) =>
          contains(`${track.performer} - ${track.title}`, search),
        ),
    [playlistTracks, contains, search],
  )

  const filteredTracks = useMemo(
    () => filteredIndexedTracks.map(({ track }) => track),
    [filteredIndexedTracks],
  )

  const filteredSourceIndices = useMemo(
    () => filteredIndexedTracks.map(({ sourceIndex }) => sourceIndex),
    [filteredIndexedTracks],
  )

  const likedTrackIds = useMemo(() => getLikedTrackIdSet(data), [data])
  const selectedSourceIndices = useMemo(
    () =>
      Object.keys(rowSelection)
        .filter(id => rowSelection[id])
        .map(Number)
        .filter(id => Number.isFinite(id))
        .sort((a, b) => a - b),
    [rowSelection],
  )
  const selectedTrackIds = useMemo(
    () =>
      selectedSourceIndices
        .map(index => playlistTracks[index]?.id)
        .filter((id): id is number => id !== undefined),
    [playlistTracks, selectedSourceIndices],
  )

  const canReorder
    = selectedPlaylistId !== ALL_TRACKS_PLAYLIST_ID
      && selectedPlaylistId !== POPULAR_PLAYLIST_ID
      && selectedPlaylistId !== RECENT_PLAYLIST_ID
      && search.length === 0
      && sorting.length === 0
      && !selectionMode

  const playableEntries = useMemo(
    () => sortIndexedPlaylistTracks(
      playlistTracks,
      sortingStateToTrackSort(sorting),
    ),
    [playlistTracks, sorting],
  )

  const customPlaylists = data?.custom ?? []

  const handleTrackSelect = (track: Track, sourceIndex: number) => {
    // Same membership row: toggle pause/resume instead of restarting.
    if (playingSourceIndex === sourceIndex) {
      setPlaying(!isPlaying)
      return
    }

    // Search filters the table only — queue is still the full playlist.
    // Column sort does apply to playback order; start at this membership slot.
    const startIndex = playableEntries.findIndex(
      entry => entry.sourceIndex === sourceIndex,
    )
    playPlaylist(selectedPlaylist, {
      start: track,
      startIndex: startIndex >= 0 ? startIndex : 0,
      orderedEntries: playableEntries,
    })
  }

  const handleReorderTracks = async (
    trackIds: number[],
    move: { fromIndex: number, toIndex: number },
  ) => {
    const latest = usePlaylistsStore.getState().data
    if (!latest) return

    const dbPlaylistId
      = selectedPlaylistId === LIKED_PLAYLIST_ID
        ? latest.liked.id
        : typeof selectedPlaylistId === 'number'
          ? selectedPlaylistId
          : null
    if (dbPlaylistId === null) return

    const previousTrackIds
      = selectedPlaylistId === LIKED_PLAYLIST_ID
        ? latest.liked.trackIds
        : latest.custom.find(playlist => playlist.id === selectedPlaylistId)
          ?.trackIds
    if (!previousTrackIds) return

    const trackIdsMatch = (current: number[]) =>
      current.length === trackIds.length
      && current.every((id, index) => id === trackIds[index])

    const resolveTracks = (ids: number[]) => {
      const trackById = new Map(libraryTracks.map(track => [track.id, track]))
      return ids
        .map(id => trackById.get(id))
        .filter((track): track is Track => track !== undefined)
    }

    const realignQueue = (
      ids: number[],
      reorderMove: { fromIndex: number, toIndex: number },
    ) => {
      usePlayerStore.getState().realignQueueToPlaylist(
        selectedPlaylistId,
        resolveTracks(ids),
        reorderMove,
      )
    }

    if (selectedPlaylistId === LIKED_PLAYLIST_ID) {
      setData({
        ...latest,
        liked: { ...latest.liked, trackIds },
      })
    }
    else {
      setData({
        ...latest,
        custom: latest.custom.map(playlist =>
          playlist.id === selectedPlaylistId
            ? { ...playlist, trackIds }
            : playlist,
        ),
      })
    }
    realignQueue(trackIds, move)

    try {
      const updatedAt = await api.reorderPlaylistTracks(dbPlaylistId, trackIds)
      const after = usePlaylistsStore.getState().data
      if (!after) return
      // Only stamp updatedAt if this playlist still has our optimistic order.
      if (selectedPlaylistId === LIKED_PLAYLIST_ID) {
        if (!trackIdsMatch(after.liked.trackIds)) return
        setData({
          ...after,
          liked: { ...after.liked, updatedAt },
        })
      }
      else {
        const current = after.custom.find(
          playlist => playlist.id === selectedPlaylistId,
        )
        if (!current || !trackIdsMatch(current.trackIds)) return
        setData({
          ...after,
          custom: after.custom.map(playlist =>
            playlist.id === selectedPlaylistId
              ? { ...playlist, updatedAt }
              : playlist,
          ),
        })
      }
    }
    catch {
      const after = usePlaylistsStore.getState().data
      if (!after) return
      const reverseMove = {
        fromIndex: move.toIndex,
        toIndex: move.fromIndex,
      }
      // Roll back only this playlist, and only if nothing else changed its order.
      if (selectedPlaylistId === LIKED_PLAYLIST_ID) {
        if (!trackIdsMatch(after.liked.trackIds)) return
        setData({
          ...after,
          liked: { ...after.liked, trackIds: previousTrackIds },
        })
        realignQueue(previousTrackIds, reverseMove)
      }
      else {
        const current = after.custom.find(
          playlist => playlist.id === selectedPlaylistId,
        )
        if (!current || !trackIdsMatch(current.trackIds)) return
        setData({
          ...after,
          custom: after.custom.map(playlist =>
            playlist.id === selectedPlaylistId
              ? { ...playlist, trackIds: previousTrackIds }
              : playlist,
          ),
        })
        realignQueue(previousTrackIds, reverseMove)
      }
    }
  }

  const handleEnterSelection = (sourceIndex: number) => {
    const next = enterSelectionWithTrack(sourceIndex)
    setSelectionMode(next.selectionMode)
    setRowSelection(next.rowSelection)
  }

  const handleExitSelection = () => {
    setSelectionMode(false)
    setRowSelection({})
  }

  const handleToggleLike = async (trackId: number) => {
    if (!usePlaylistsStore.getState().data) return
    try {
      const liked = await api.toggleLike(trackId)
      const latest = usePlaylistsStore.getState().data
      if (!latest) return
      setData({ ...latest, liked })
    }
    catch {
      // keep UI unchanged on failure
    }
  }

  const handleBulkAddToLiked = async (trackIds: number[]) => {
    for (const trackId of trackIds) {
      await handleToggleLike(trackId)
    }
  }

  const handleBulkRemoveFromLiked = async (trackIds: number[]) => {
    for (const trackId of trackIds) {
      await handleToggleLike(trackId)
    }
    setRowSelection({})
  }

  const handleAddToPlaylist = async (targetId: number, trackId: number) => {
    if (!usePlaylistsStore.getState().data) return
    try {
      const updatedAt = await api.addTrackToPlaylist(targetId, trackId)
      const latest = usePlaylistsStore.getState().data
      if (!latest) return
      setData({
        ...latest,
        custom: latest.custom.map(playlist =>
          playlist.id === targetId
            ? {
                ...playlist,
                updatedAt,
                trackIds: [...playlist.trackIds, trackId],
              }
            : playlist,
        ),
      })
    }
    catch {
      // keep UI unchanged on failure
    }
  }

  const handleBulkAddToPlaylist = async (
    targetId: number,
    trackIds: number[],
  ) => {
    if (!usePlaylistsStore.getState().data || trackIds.length === 0) return
    try {
      const updatedAt = await api.addTracksToPlaylist(targetId, trackIds)
      const latest = usePlaylistsStore.getState().data
      if (!latest) return
      setData({
        ...latest,
        custom: latest.custom.map(playlist =>
          playlist.id === targetId
            ? {
                ...playlist,
                updatedAt,
                trackIds: [...playlist.trackIds, ...trackIds],
              }
            : playlist,
        ),
      })
    }
    catch {
      // keep UI unchanged on failure
    }
  }

  const handleDeleteFromPlaylist = async (
    targetId: CustomPlaylistId,
    position: number,
  ) => {
    if (!usePlaylistsStore.getState().data) return
    try {
      const updatedAt = await api.removeTrackFromPlaylist(targetId, position)
      const latest = usePlaylistsStore.getState().data
      if (!latest) return
      setData({
        ...latest,
        custom: latest.custom.map(playlist =>
          playlist.id === targetId
            ? {
                ...playlist,
                updatedAt,
                trackIds: playlist.trackIds.filter((_, index) => index !== position),
              }
            : playlist,
        ),
      })
    }
    catch {
      // keep UI unchanged on failure
    }
  }

  const handleBulkRemoveFromPlaylist = async (
    targetId: number,
    positions: number[],
  ) => {
    const descending = [...positions].sort((a, b) => b - a)
    for (const position of descending) {
      await handleDeleteFromPlaylist(targetId, position)
    }
    setRowSelection({})
  }

  const handlePlayNext = (track: Track) => {
    enqueueNext([track])
  }

  const handleAddToEnd = (track: Track) => {
    appendToQueue([track])
  }

  const handleBulkPlayNext = () => {
    enqueueNext(
      selectedSourceIndices
        .map(index => playlistTracks[index])
        .filter((track): track is Track => track !== undefined),
    )
  }

  const handleBulkAddToEnd = () => {
    appendToQueue(
      selectedSourceIndices
        .map(index => playlistTracks[index])
        .filter((track): track is Track => track !== undefined),
    )
  }

  const handleCache = async (track: Track) => {
    const cache = useCacheStore.getState()
    if (cache.isBusy(track.id)) return
    cache.markBusy([track.id])
    try {
      await api.cacheTrack(track.id)
      useCacheStore.getState().markCached([track.id])
    }
    catch (error) {
      setActionError(errorMessage(error))
    }
    finally {
      useCacheStore.getState().clearBusy([track.id])
    }
  }

  const handleBulkCache = async (trackIds: number[]) => {
    if (trackIds.length === 0) return
    const cache = useCacheStore.getState()
    const ids = trackIds.filter(id => !cache.isBusy(id))
    if (ids.length === 0) return
    cache.markBusy(ids)
    try {
      const cached = await api.cacheTracks(ids)
      useCacheStore.getState().markCached(cached)
    }
    catch (error) {
      setActionError(errorMessage(error))
    }
    finally {
      useCacheStore.getState().clearBusy(ids)
    }
  }

  const handleCachePlaylist = async () => {
    const trackIds = playlistTracks.map(track => track.id)
    await handleBulkCache(trackIds)
  }

  const handleDownload = async (track: Track) => {
    const cache = useCacheStore.getState()
    if (cache.isBusy(track.id)) return
    cache.markBusy([track.id])
    try {
      const path = await api.exportTrack(track.id)
      await revealItemInDir(path)
    }
    catch (error) {
      setActionError(errorMessage(error))
    }
    finally {
      useCacheStore.getState().clearBusy([track.id])
    }
  }

  const handleBulkDownload = async (trackIds: number[]) => {
    if (trackIds.length === 0) return
    const cache = useCacheStore.getState()
    const ids = trackIds.filter(id => !cache.isBusy(id))
    if (ids.length === 0) return
    cache.markBusy(ids)
    try {
      await api.exportTracks(ids)
    }
    catch (error) {
      setActionError(errorMessage(error))
    }
    finally {
      useCacheStore.getState().clearBusy(ids)
    }
  }

  const handleRemoveFromCache = async (track: Track) => {
    try {
      await api.removeTrackFromCache(track.id)
      useCacheStore.getState().markUncached([track.id])
    }
    catch (error) {
      setActionError(errorMessage(error))
    }
  }

  function handlePlaylistPlay() {
    playPlaylist(selectedPlaylist, {
      startIndex: 0,
      orderedEntries: playableEntries,
    })
  }

  function handlePlaylistShuffle() {
    playPlaylist(selectedPlaylist, {
      shuffle: 'on',
      orderedEntries: playableEntries,
    })
  }

  const handleShowInfo = (track: Track) => {
    setInfoTrack(track)
  }

  const handleInfoOpenChange = (open: boolean) => {
    if (!open) {
      setInfoTrack(null)
    }
  }

  const handleActionErrorOpenChange = (open: boolean) => {
    if (!open) setActionError(null)
  }

  const checkTrackLiked = (trackId: number) => isTrackLiked(data, trackId)
  const cachedIds = useCacheStore(state => state.cachedIds)
  const playlistCached
    = playlistTracks.length > 0
      && playlistTracks.every(track => cachedIds.has(track.id))

  return {
    search,
    setSearch,
    selectionMode,
    rowSelection,
    setRowSelection,
    sorting,
    setSorting,
    infoTrack,
    actionError,
    libraryTrackCount: libraryTracks.length,
    playlistTracks,
    isCustom,
    playlistId,
    selectedPlaylist,
    customPlaylists,
    playingSourceIndex,
    isPlaying,
    filteredTracks,
    filteredSourceIndices,
    likedTrackIds,
    selectedSourceIndices,
    selectedTrackIds,
    canReorder,
    playlistCached,
    checkTrackLiked,
    handleTrackSelect,
    handleReorderTracks,
    handleEnterSelection,
    handleExitSelection,
    handleToggleLike,
    handleBulkAddToLiked,
    handleBulkRemoveFromLiked,
    handleAddToPlaylist,
    handleBulkAddToPlaylist,
    handleDeleteFromPlaylist,
    handleBulkRemoveFromPlaylist,
    handlePlayNext,
    handleAddToEnd,
    handleBulkPlayNext,
    handleBulkAddToEnd,
    handleCache,
    handleBulkCache,
    handleCachePlaylist,
    handleDownload,
    handleBulkDownload,
    handleRemoveFromCache,
    handlePlaylistPlay,
    handlePlaylistShuffle,
    handleShowInfo,
    handleInfoOpenChange,
    handleActionErrorOpenChange,
  }
}
