import type { Track } from '@/types'
import { captureSession, isSessionCurrent } from '@/stores/session-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { usePlayerStore } from '@/stores/player-store'
import { useLibraryStore } from '@/stores/library-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { resolveSelectedPlaylistTracks, type PlaylistEntry, type ResolvedSelectedPlaylist } from '@/lib/playlists'
import { reorderPlaylistAndQueue } from '@/lib/playlist-playback'
import { errorMessage } from '@/lib/errors'
import { useTrackFileActions } from '@/hooks/use-track-file-actions'

interface ActionSelection {
  selectedEntries: PlaylistEntry[]
  exit: () => void
  clear: () => void
  retainFailed: (playlist: ResolvedSelectedPlaylist, ids: number[]) => void
}

export function usePlaylistActions(
  playlist: ResolvedSelectedPlaylist,
  playableEntries: PlaylistEntry[],
  selection: ActionSelection,
  onError: (message: string) => void,
  onShowInfo: (track: Track) => void,
) {
  const report = async (operation: () => Promise<unknown>) => {
    const generation = captureSession()
    try {
      await operation()
      return isSessionCurrent(generation)
    }
    catch (error) {
      if (isSessionCurrent(generation)) onError(errorMessage(error))
      return false
    }
  }
  const playlists = usePlaylistsStore.getState()
  const player = usePlayerStore.getState()
  const files = useTrackFileActions(onError)
  const setLiked = (ids: number[], liked: boolean) => report(async () => {
    const { failedTrackIds } = await playlists.setLiked(ids, liked)
    if (!failedTrackIds.length) {
      selection.exit()
      return
    }
    const latest = resolveSelectedPlaylistTracks(useLibraryStore.getState().library,
      usePlaylistsStore.getState().data, playlist.id, useListenStatsStore.getState().statsByTrackId)
    selection.retainFailed(latest, failedTrackIds)
    throw new Error(`Could not update ${failedTrackIds.length} selected tracks. They remain selected for retry.`)
  })
  const selectedTracks = () => selection.selectedEntries.map(entry => entry.track)
  return {
    track: {
      onToggleLike: (id: number) => report(() => playlists.toggleLike(id)),
      onAddToPlaylist: (id: number, trackId: number) => report(() => playlists.addTracks(id, [trackId])),
      onDeleteFromPlaylist: (id: number, position: number) => report(() => playlists.removeTracks(id, [position], playlist.trackIds)),
      onPlayNext: (track: Track) => player.enqueueNext([track]),
      onAddToEnd: (track: Track) => player.appendToQueue([track]),
      onShowInfo, onCache: files.handleCache, onDownload: files.handleDownload, onRemoveFromCache: files.handleRemoveFromCache,
    },
    bulk: {
      onAddToLiked: (ids: number[]) => setLiked(ids, true),
      onRemoveFromLiked: (ids: number[]) => setLiked(ids, false),
      onAddToPlaylist: (id: number, ids: number[]) => report(() => playlists.addTracks(id, ids)),
      onRemoveFromPlaylist: async (id: number, positions: number[]) => {
        if (await report(() => playlists.removeTracks(id, positions, playlist.trackIds))) selection.clear()
      },
      onPlayNext: () => player.enqueueNext(selectedTracks()),
      onAddToEnd: () => player.appendToQueue(selectedTracks()),
      onCache: files.handleBulkCache, onDownload: files.handleBulkDownload,
    },
    reorder: (ids: number[], move: { fromIndex: number, toIndex: number }) => report(() => reorderPlaylistAndQueue(playlist.id, ids, playlist.trackIds, move)),
    selectTrack: (track: Track, sourceIndex: number) => player.playPlaylist(playlist, {
      toggleIfCurrent: true, start: track,
      startIndex: Math.max(0, playableEntries.findIndex(entry => entry.sourceIndex === sourceIndex)),
      orderedEntries: playableEntries,
    }),
    play: () => player.playPlaylist(playlist, { startIndex: 0, orderedEntries: playableEntries }),
    shuffle: () => player.playPlaylist(playlist, { shuffle: 'on', orderedEntries: playableEntries }),
  }
}
