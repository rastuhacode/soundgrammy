import { useMemo, useState } from 'react'
import type { SortingState } from '@tanstack/react-table'
import type { Track } from '@/types'
import { canReorderPlaylist } from '@/lib/playlists'
import { canDownloadPlaylist } from '@/lib/playlist-track-actions'
import { resolvePlayingSourceIndex } from '@/lib/queue/playing-source-index'
import { usePlayerStore } from '@/stores/player-store'
import { useCacheStore } from '@/stores/cache-store'
import { usePlaylistJobsStore } from '@/stores/playlist-jobs-store'
import { usePlaylistTracks } from '@/hooks/use-playlist-tracks'
import { usePlaylistSelection } from '@/hooks/use-playlist-selection'
import { usePlaylistActions } from '@/hooks/use-playlist-actions'

/** Compose focused domain hooks into the playlist screen's presentation model. */
export function usePlaylistView() {
  const [search, setSearch] = useState('')
  const [sorting, setSorting] = useState<SortingState>([])
  const [infoTrack, setInfoTrack] = useState<Track | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [createTrackIds, setCreateTrackIds] = useState<number[] | null>(null)
  const tracks = usePlaylistTracks(search, sorting)
  const { playlist } = tracks
  const selection = usePlaylistSelection(playlist, tracks.entries)
  const actions = usePlaylistActions(playlist, tracks.playableEntries, selection, setActionError, setInfoTrack)
  const queue = usePlayerStore(state => state.queue)
  const currentTrackId = usePlayerStore(state => state.currentTrack?.id ?? null)
  const isPlaying = usePlayerStore(state => state.isPlaying)
  const cachedIds = useCacheStore(state => state.cachedIds)
  const downloadJob = usePlaylistJobsStore(state => state.jobsById[state.downloadJobByPlaylist[String(playlist.id)] ?? ''] ?? null)
  const cacheJob = usePlaylistJobsStore(state => state.jobsById[state.cacheJobByPlaylist[String(playlist.id)] ?? ''] ?? null)
  const playingSourceIndex = useMemo(() => resolvePlayingSourceIndex({
    currentTrackId, playlistId: playlist.id, playlistTrackIds: playlist.trackIds,
    queue: { cursor: queue.cursor, source: queue.source, sourceIndices: queue.sourceIndices, trackIds: queue.tracks.map(track => track.id) },
  }), [currentTrackId, playlist.id, playlist.trackIds, queue])
  const jobInput = { playlistId: playlist.id, name: playlist.name, trackIds: playlist.tracks.map(track => track.id) }
  return {
    playlist,
    libraryCount: tracks.libraryCount,
    search: { value: search, setValue: setSearch },
    selection,
    table: {
      entries: tracks.filteredEntries, currentPlaylist: playlist, customPlaylists: tracks.custom,
      playingSourceIndex, isPlaying, isTrackLiked: (id: number) => tracks.likedIds.has(id),
      selectionMode: selection.mode, rowSelection: selection.rows, onRowSelectionChange: selection.setRows,
      onTrackSelect: (position: number, selected: boolean, extend: boolean) =>
        selection.select(position, tracks.filteredEntries, extend, selected),
      sorting, onSortingChange: setSorting,
      canReorder: canReorderPlaylist(playlist.id) && tracks.entries.length === playlist.trackIds.length
        && !search.length && !sorting.length && !selection.mode,
      onReorderTracks: actions.reorder, onEnterSelection: selection.enter, onTrackPlay: actions.selectTrack,
      trackActions: { ...actions.track, onCreatePlaylist: (id: number) => setCreateTrackIds([id]) },
    },
    bulkActions: {
      selectedTrackIds: selection.trackIds, selectedPositions: selection.positions,
      currentPlaylist: playlist, customPlaylists: tracks.custom, likedTrackIds: tracks.likedIds,
      onExitSelection: selection.exit, ...actions.bulk,
      onCreatePlaylist: (ids: number[]) => setCreateTrackIds([...ids]),
    },
    playbackActions: {
      currentPlaylist: playlist,
      playlistCached: playlist.tracks.length > 0 && playlist.tracks.every(track => cachedIds.has(track.id)),
      playlistDownloading: downloadJob !== null, playlistDownloadProgress: downloadJob?.progress ?? null,
      playlistCaching: cacheJob !== null, playlistCacheProgress: cacheJob?.progress ?? null,
      onPlay: actions.play, onShuffle: actions.shuffle,
      onCachePlaylist: () => usePlaylistJobsStore.getState().runCachePlaylist(jobInput),
      onDownloadPlaylist: () => {
        if (canDownloadPlaylist(playlist)) return usePlaylistJobsStore.getState().runDownloadPlaylist(jobInput)
      },
    },
    dialogs: {
      infoTrack, actionError, createTrackIds,
      onCreateOpenChange: (open: boolean) => { if (!open) setCreateTrackIds(null) },
      onInfoOpenChange: (open: boolean) => { if (!open) setInfoTrack(null) },
      onErrorOpenChange: (open: boolean) => { if (!open) setActionError(null) },
    },
  }
}
