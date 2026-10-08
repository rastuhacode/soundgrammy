import { useRef, useState } from 'react'
import { ArrowLeft, X } from 'lucide-react'
import { useCompactDisplay } from '@/hooks/use-compact-display'
import { usePlayerStore } from '@/stores/player-store'
import { Input } from '@/components/ui/input'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { usePlaylistJobsStore } from '@/stores/playlist-jobs-store'
import { usePlaylistView } from '@/hooks/use-playlist-view'
import { useAndroidBackAction } from '@/hooks/use-android-back'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { PlaylistEmptyState } from './PlaylistEmptyState'
import { PlaylistToolbar } from './PlaylistToolbar'
import { PlaylistTracksTable } from './PlaylistTracksTable'
import { PlaylistDownloadResultDialog } from './PlaylistDownloadResultDialog'
import { TrackInfoDialog } from './TrackInfoDialog'

export function PlaylistView({ onBack }: { onBack?: () => void }) {
  const selectedPlaylistId = usePlaylistsStore(
    state => state.selectedPlaylistId,
  )
  const resultItem = usePlaylistJobsStore(state => state.resultQueue[0] ?? null)
  const dismissResult = usePlaylistJobsStore(state => state.dismissResult)
  const errorMessage = usePlaylistJobsStore(state => state.errorQueue[0] ?? null)
  const dismissError = usePlaylistJobsStore(state => state.dismissError)

  const handleOpenFolder = async () => {
    if (!resultItem?.result.folderPath) return
    try {
      await revealItemInDir(resultItem.result.folderPath)
    }
    catch {
      // Ignore; user can browse Downloads manually.
    }
  }

  return (
    <>
      <PlaylistViewContent key={selectedPlaylistId} onBack={onBack} />

      <PlaylistDownloadResultDialog
        result={resultItem?.result ?? null}
        playlistName={resultItem?.playlistName ?? null}
        open={resultItem !== null}
        onOpenChange={(open) => {
          if (!open) dismissResult()
        }}
        onOpenFolder={handleOpenFolder}
      />

      <Dialog
        open={errorMessage !== null}
        onOpenChange={(open) => {
          if (!open) dismissError()
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Couldn’t finish</DialogTitle>
            <DialogDescription className="whitespace-pre-wrap">
              {errorMessage}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="secondary" onClick={() => dismissError()}>
              OK
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

function PlaylistViewContent({ onBack }: { onBack?: () => void }) {
  const view = usePlaylistView()
  const [searchOpen, setSearchOpen] = useState(false)
  const searchInputRef = useRef<HTMLInputElement>(null)
  const searchButtonRef = useRef<HTMLButtonElement>(null)
  const { isCompact } = useCompactDisplay()
  const [previousCompact, setPreviousCompact] = useState(isCompact)
  const hasPlayer = usePlayerStore(state => state.currentTrack !== null)
  const hasTracks = view.playlistTracks.length > 0
  useAndroidBackAction(view.selectionMode, view.handleExitSelection)

  // Search belongs to the current layout; resizing must not carry a modal query
  // into the desktop playlist, or leave the compact playlist filtered behind it.
  if (previousCompact !== isCompact) {
    setPreviousCompact(isCompact)
    setSearchOpen(false)
    view.setSearch('')
  }

  const handleSearchOpenChange = (open: boolean) => {
    setSearchOpen(open)
    if (!open) view.setSearch('')
  }

  const trackTable = (
    <PlaylistTracksTable
      hideHeader={searchOpen}
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
      onCache={view.handleCache}
      onDownload={view.handleDownload}
      onRemoveFromCache={view.handleRemoveFromCache}
      onShowInfo={view.handleShowInfo}
    />
  )

  return (
    <>
      <div className="flex min-h-0 grow flex-col gap-3 md:gap-4 md:pt-4">
        {(hasTracks || onBack) && (
          <PlaylistToolbar
            onBack={onBack}
            hasTracks={hasTracks}
            search={view.search}
            onSearchChange={view.setSearch}
            onOpenSearch={() => handleSearchOpenChange(true)}
            searchButtonRef={searchButtonRef}
            selectionMode={view.selectionMode}
            selectedTrackIds={view.selectedTrackIds}
            selectedPositions={view.selectedSourceIndices}
            currentPlaylist={view.selectedPlaylist}
            customPlaylists={view.customPlaylists}
            likedTrackIds={view.likedTrackIds}
            playlistCached={view.playlistCached}
            playlistDownloading={view.playlistDownloading}
            playlistDownloadProgress={view.playlistDownloadProgress}
            playlistCaching={view.playlistCaching}
            playlistCacheProgress={view.playlistCacheProgress}
            onPlay={view.handlePlaylistPlay}
            onShuffle={view.handlePlaylistShuffle}
            onCachePlaylist={view.handleCachePlaylist}
            onDownloadPlaylist={view.handleDownloadPlaylist}
            onExitSelection={view.handleExitSelection}
            onAddToLiked={view.handleBulkAddToLiked}
            onRemoveFromLiked={view.handleBulkRemoveFromLiked}
            onAddToPlaylist={view.handleBulkAddToPlaylist}
            onRemoveFromPlaylist={view.handleBulkRemoveFromPlaylist}
            onPlayNext={view.handleBulkPlayNext}
            onAddToEnd={view.handleBulkAddToEnd}
            onCache={view.handleBulkCache}
            onDownload={view.handleBulkDownload}
          />
        )}

        {!searchOpen && (hasTracks
          ? (
              trackTable
            )
          : (
              <PlaylistEmptyState
                libraryTrackCount={view.libraryTrackCount}
                playlistId={view.playlistId}
                isCustom={view.isCustom}
              />
            ))}
      </div>

      <Dialog open={isCompact && searchOpen} onOpenChange={handleSearchOpenChange} modal={false} disablePointerDismissal>
        <DialogContent
          className="playlist-search-overlay flex flex-col gap-0 overflow-hidden p-0"
          showCloseButton={false}
          overlayClassName="hidden"
          initialFocus={searchInputRef}
          finalFocus={searchButtonRef}
          style={{ height: hasPlayer ? 'calc(100dvh - 6rem)' : '100dvh' }}
        >
          <DialogTitle className="sr-only">Search tracks</DialogTitle>
          <header className="android-overlay-inset flex shrink-0 items-center gap-2 border-b border-border px-3 py-3">
            <Button
              variant="ghost"
              size="icon"
              aria-label="Close track search"
              onClick={() => handleSearchOpenChange(false)}
            >
              <ArrowLeft className="size-5" />
            </Button>
            <Input
              ref={searchInputRef}
              type="search"
              aria-label="Search tracks"
              placeholder="Search tracks"
              value={view.search}
              onChange={event => view.setSearch(event.target.value)}
              className="min-w-0 flex-1 border-none bg-transparent text-base shadow-none focus-visible:ring-0 dark:bg-transparent [&::-webkit-search-cancel-button]:hidden"
            />
            {view.search && (
              <Button
                variant="ghost"
                size="icon"
                aria-label="Clear track search"
                onClick={() => {
                  view.setSearch('')
                  searchInputRef.current?.focus()
                }}
              >
                <X className="size-5" />
              </Button>
            )}
          </header>
          <p className="shrink-0 px-5 py-3 text-sm text-muted-foreground" aria-live="polite">
            {view.filteredTracks.length}
            {' '}
            {view.filteredTracks.length === 1 ? 'track' : 'tracks'}
            {' '}
            in
            {' '}
            {view.selectedPlaylist.name}
          </p>
          {view.filteredTracks.length > 0
            ? trackTable
            : <p className="px-5 py-6 text-sm text-muted-foreground">No tracks found.</p>}
        </DialogContent>
      </Dialog>

      <TrackInfoDialog
        track={view.infoTrack}
        open={view.infoTrack !== null}
        onOpenChange={view.handleInfoOpenChange}
      />

      <Dialog
        open={view.actionError !== null}
        onOpenChange={view.handleActionErrorOpenChange}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Couldn’t finish</DialogTitle>
            <DialogDescription className="whitespace-pre-wrap">
              {view.actionError}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="secondary"
              onClick={() => view.handleActionErrorOpenChange(false)}
            >
              OK
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
