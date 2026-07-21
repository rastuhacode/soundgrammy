import { usePlaylistsStore } from '@/stores/playlists-store'
import { usePlaylistView } from '@/hooks/use-playlist-view'
import { PlaylistEmptyState } from './PlaylistEmptyState'
import { PlaylistToolbar } from './PlaylistToolbar'
import { PlaylistTracksTable } from './PlaylistTracksTable'
import { TrackInfoDialog } from './TrackInfoDialog'

export function PlaylistView() {
  const selectedPlaylistId = usePlaylistsStore(
    state => state.selectedPlaylistId,
  )

  // Remount on playlist change so search / selection reset without an effect.
  return <PlaylistViewContent key={selectedPlaylistId} />
}

function PlaylistViewContent() {
  const view = usePlaylistView()

  if (view.playlistTracks.length === 0) {
    return (
      <PlaylistEmptyState
        libraryTrackCount={view.libraryTrackCount}
        playlistId={view.playlistId}
        isCustom={view.isCustom}
      />
    )
  }

  return (
    <>
      <div className="flex min-h-0 grow flex-col gap-4 pt-4">
        <PlaylistToolbar
          search={view.search}
          onSearchChange={view.setSearch}
          selectionMode={view.selectionMode}
          selectedTrackIds={view.selectedTrackIds}
          selectedPositions={view.selectedSourceIndices}
          currentPlaylist={view.selectedPlaylist}
          customPlaylists={view.customPlaylists}
          likedTrackIds={view.likedTrackIds}
          onPlay={view.handlePlaylistPlay}
          onShuffle={view.handlePlaylistShuffle}
          onExitSelection={view.handleExitSelection}
          onAddToLiked={view.handleBulkAddToLiked}
          onRemoveFromLiked={view.handleBulkRemoveFromLiked}
          onAddToPlaylist={view.handleBulkAddToPlaylist}
          onRemoveFromPlaylist={view.handleBulkRemoveFromPlaylist}
          onPlayNext={view.handleBulkPlayNext}
          onAddToEnd={view.handleBulkAddToEnd}
          onDownload={view.handleBulkDownload}
        />

        <PlaylistTracksTable
          tracks={view.filteredTracks}
          sourceIndices={view.filteredSourceIndices}
          currentPlaylist={view.selectedPlaylist}
          customPlaylists={view.customPlaylists}
          playingSourceIndex={view.playingSourceIndex}
          isPlaying={view.isPlaying}
          isTrackLiked={view.checkTrackLiked}
          selectionMode={view.selectionMode}
          rowSelection={view.rowSelection}
          onRowSelectionChange={view.setRowSelection}
          sorting={view.sorting}
          onSortingChange={view.setSorting}
          canReorder={view.canReorder}
          onReorderTracks={view.handleReorderTracks}
          onEnterSelection={view.handleEnterSelection}
          onTrackPlay={view.handleTrackSelect}
          onToggleLike={view.handleToggleLike}
          onAddToPlaylist={view.handleAddToPlaylist}
          onDeleteFromPlaylist={view.handleDeleteFromPlaylist}
          onPlayNext={view.handlePlayNext}
          onAddToEnd={view.handleAddToEnd}
          onDownload={view.handleDownload}
          onShowInfo={view.handleShowInfo}
        />
      </div>

      <TrackInfoDialog
        track={view.infoTrack}
        open={view.infoTrack !== null}
        onOpenChange={view.handleInfoOpenChange}
      />
    </>
  )
}
