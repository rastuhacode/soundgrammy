import { captureSession, isSessionCurrent } from '@/stores/session-store'
import { useTrackFileActions } from '@/hooks/use-track-file-actions'
import { playlistMembershipKey, playlistMembershipIndices, canReorderPlaylist } from '@/lib/playlists'
import type { OnChangeFn, RowSelectionState, SortingState } from '@tanstack/react-table'
import { errorMessage } from '@/lib/errors'
import type { Track } from '@/lib/db'
import { useLibraryStore } from '@/stores/library-store'
import { usePlayerStore } from '@/stores/player-store'
import {
  getLikedTrackIdSet,
  resolveSelectedPlaylistTracks,
  usePlaylistsStore,
} from '@/stores/playlists-store'
import type { CustomPlaylistId } from '@/stores/playlists-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { useMemo, useState } from 'react'
import { resolvePlayingSourceIndex } from '@/lib/queue/playing-source-index'
import { useFilter } from '@/hooks/utils/use-filter'
import {
  canDownloadPlaylist,
  enterSelectionWithTrack,
  sortIndexedPlaylistTracks,
  sortingStateToTrackSort,
} from '@/lib/playlist-track-actions'
import { useCacheStore } from '@/stores/cache-store'
import { usePlaylistJobsStore } from '@/stores/playlist-jobs-store'

export function usePlaylistView() {
  const [search, setSearch] = useState('')
  const [selection, setSelection] = useState<{ key: string, rows: RowSelectionState } | null>(null)
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
  const enqueueNext = usePlayerStore(state => state.enqueueNext)
  const appendToQueue = usePlayerStore(state => state.appendToQueue)
  const data = usePlaylistsStore(state => state.data)
  const selectedPlaylistId = usePlaylistsStore(
    state => state.selectedPlaylistId,
  )

  const downloadJob = usePlaylistJobsStore((state) => {
    const jobId = state.downloadJobByPlaylist[String(selectedPlaylistId)]
    return jobId ? state.jobsById[jobId] ?? null : null
  })
  const cacheJob = usePlaylistJobsStore((state) => {
    const jobId = state.cacheJobByPlaylist[String(selectedPlaylistId)]
    return jobId ? state.jobsById[jobId] ?? null : null
  })
  const runDownloadPlaylist = usePlaylistJobsStore(
    state => state.runDownloadPlaylist,
  )
  const runCachePlaylist = usePlaylistJobsStore(
    state => state.runCachePlaylist,
  )

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

  const membershipKey = useMemo(() => playlistMembershipKey(selectedPlaylist), [selectedPlaylist])
  const membershipIndices = useMemo(() => playlistMembershipIndices(selectedPlaylist), [selectedPlaylist])
  const selectionMode = selection?.key === membershipKey
  const rowSelection = useMemo<RowSelectionState>(() => selectionMode ? selection!.rows : {}, [selection, selectionMode])
  if (selection && !selectionMode) setSelection(null)
  const setRowSelection: OnChangeFn<RowSelectionState> = (update) => {
    setSelection(previous => ({ key: membershipKey, rows: typeof update === 'function'
      ? update(previous?.key === membershipKey ? previous.rows : {})
      : update }))
  }

  const playingSourceIndex = useMemo(() => {
    return resolvePlayingSourceIndex({
      currentTrackId,
      playlistId,
      playlistTrackIds: selectedPlaylist.trackIds,
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
    selectedPlaylist.trackIds,
    queue.cursor,
    queue.source,
    queue.sourceIndices,
    queue.tracks,
  ])

  const filteredIndexedTracks = useMemo(
    () =>
      playlistTracks
        .map((track, index) => ({ track, sourceIndex: membershipIndices[index]! }))
        .filter(({ track }) =>
          contains(`${track.performer} - ${track.title}`, search),
        ),
    [playlistTracks, membershipIndices, contains, search],
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
        .map(index => playlistTracks[membershipIndices.indexOf(index)]?.id)
        .filter((id): id is number => id !== undefined),
    [playlistTracks, membershipIndices, selectedSourceIndices],
  )

  const canReorder = canReorderPlaylist(selectedPlaylistId)
    && playlistTracks.length === selectedPlaylist.trackIds.length
    && search.length === 0 && sorting.length === 0 && !selectionMode

  const playableEntries = useMemo(
    () => sortIndexedPlaylistTracks(
      playlistTracks,
      sortingStateToTrackSort(sorting),
      membershipIndices,
    ),
    [playlistTracks, membershipIndices, sorting],
  )

  const customPlaylists = data?.custom ?? []

  const handleTrackSelect = (track: Track, sourceIndex: number) => {
    // Search filters the table only — queue is still the full playlist.
    // Column sort does apply to playback order; start at this membership slot.
    const startIndex = playableEntries.findIndex(
      entry => entry.sourceIndex === sourceIndex,
    )
    playPlaylist(selectedPlaylist, {
      toggleIfCurrent: true,
      start: track,
      startIndex: startIndex >= 0 ? startIndex : 0,
      orderedEntries: playableEntries,
    })
  }

  const reportAction = async (operation: () => Promise<unknown>) => {
    const generation = captureSession()
    try {
      await operation()
      return true
    }
    catch (error) {
      if (isSessionCurrent(generation)) setActionError(errorMessage(error))
      return false
    }
  }

  const handleReorderTracks = (trackIds: number[], move: { fromIndex: number, toIndex: number }) =>
    reportAction(() => usePlaylistsStore.getState().reorderTracks(playlistId, trackIds, selectedPlaylist.trackIds, move))

  const handleEnterSelection = (sourceIndex: number) => {
    const next = enterSelectionWithTrack(sourceIndex)
    setRowSelection(next.rowSelection)
  }

  const handleExitSelection = () => {
    setSelection(null)
  }

  const handleToggleLike = (trackId: number) =>
    reportAction(() => usePlaylistsStore.getState().toggleLike(trackId))

  const handleBulkLiked = async (trackIds: number[], liked: boolean) => {
    await reportAction(async () => {
      const { failedTrackIds } = await usePlaylistsStore.getState().setLiked(trackIds, liked)
      const latest = resolveSelectedPlaylistTracks(
        useLibraryStore.getState().library,
        usePlaylistsStore.getState().data,
        playlistId,
        useListenStatsStore.getState().statsByTrackId,
      )
      if (!failedTrackIds.length) {
        setSelection(null)
        return
      }
      const failed = new Set(failedTrackIds)
      setSelection({ key: playlistMembershipKey(latest), rows: Object.fromEntries(
        latest.tracks.flatMap((track, index) => failed.has(track.id) ? [[String(playlistMembershipIndices(latest)[index]), true]] : []),
      ) })
      if (failedTrackIds.length) throw new Error(`Could not update ${failedTrackIds.length} selected tracks. They remain selected for retry.`)
    })
  }
  const handleBulkAddToLiked = (trackIds: number[]) => handleBulkLiked(trackIds, true)
  const handleBulkRemoveFromLiked = (trackIds: number[]) => handleBulkLiked(trackIds, false)
  const handleAddToPlaylist = (targetId: number, trackId: number) =>
    reportAction(() => usePlaylistsStore.getState().addTracks(targetId, [trackId]))
  const handleBulkAddToPlaylist = (targetId: number, trackIds: number[]) =>
    reportAction(() => usePlaylistsStore.getState().addTracks(targetId, trackIds))
  const handleDeleteFromPlaylist = (targetId: CustomPlaylistId, position: number) =>
    reportAction(() => usePlaylistsStore.getState().removeTracks(targetId, [position], selectedPlaylist.trackIds))
  const handleBulkRemoveFromPlaylist = async (targetId: number, positions: number[]) => {
    if (await reportAction(() => usePlaylistsStore.getState().removeTracks(targetId, positions, selectedPlaylist.trackIds))) {
      setRowSelection({})
    }
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
        .map(index => playlistTracks[membershipIndices.indexOf(index)])
        .filter((track): track is Track => track !== undefined),
    )
  }

  const handleBulkAddToEnd = () => {
    appendToQueue(
      selectedSourceIndices
        .map(index => playlistTracks[membershipIndices.indexOf(index)])
        .filter((track): track is Track => track !== undefined),
    )
  }

  const fileActions = useTrackFileActions(setActionError)
  const handleCachePlaylist = () => runCachePlaylist({ playlistId, name: selectedPlaylist.name, trackIds: playlistTracks.map(track => track.id) })
  const handleDownloadPlaylist = () => {
    if (!canDownloadPlaylist(selectedPlaylist) || !playlistTracks.length) return
    return runDownloadPlaylist({ playlistId, name: selectedPlaylist.name, trackIds: playlistTracks.map(track => track.id) })
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

  const checkTrackLiked = (trackId: number) => likedTrackIds.has(trackId)
  const cachedIds = useCacheStore(state => state.cachedIds)
  const playlistCached
    = playlistTracks.length > 0
      && playlistTracks.every(track => cachedIds.has(track.id))

  return {
    trackActions: {
      onToggleLike: handleToggleLike, onAddToPlaylist: handleAddToPlaylist, onDeleteFromPlaylist: handleDeleteFromPlaylist,
      onPlayNext: handlePlayNext, onAddToEnd: handleAddToEnd, onShowInfo: handleShowInfo,
      onCache: fileActions.handleCache, onDownload: fileActions.handleDownload, onRemoveFromCache: fileActions.handleRemoveFromCache,
    },
    handleBulkCache: fileActions.handleBulkCache,
    handleBulkDownload: fileActions.handleBulkDownload,
    search,
    setSearch,
    selectionMode,
    rowSelection,
    setRowSelection,
    sorting,
    setSorting,
    infoTrack,
    actionError,
    playlistDownloading: downloadJob != null,
    playlistDownloadProgress: downloadJob?.progress ?? null,
    playlistCaching: cacheJob != null,
    playlistCacheProgress: cacheJob?.progress ?? null,
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
    handleBulkAddToLiked,
    handleBulkRemoveFromLiked,
    handleBulkAddToPlaylist,
    handleBulkRemoveFromPlaylist,
    handleBulkPlayNext,
    handleBulkAddToEnd,
    handleCachePlaylist,
    handleDownloadPlaylist,
    handlePlaylistPlay,
    handlePlaylistShuffle,
    handleInfoOpenChange,
    handleActionErrorOpenChange,
  }
}
