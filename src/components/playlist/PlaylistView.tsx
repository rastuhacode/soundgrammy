import { useRef, useState } from 'react'
import { ArrowLeft, X } from 'lucide-react'
import { useCompactDisplay } from '@/hooks/use-compact-display'
import { usePlayerStore } from '@/stores/player-store'
import { Input } from '@/components/ui/input'
import { usePlaylistsStore } from '@/stores/playlists-store'
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
import { PlaylistPlaybackActions } from './PlaylistPlaybackActions'
import { PlaylistSelectionActions } from './PlaylistSelectionActions'
import { PlaylistToolbar } from './PlaylistToolbar'
import { PlaylistTracksTable } from './PlaylistTracksTable'
import { TrackInfoDialog } from './TrackInfoDialog'

export function PlaylistView({ onBack }: { onBack?: () => void }) {
  const selectedPlaylistId = usePlaylistsStore(
    state => state.selectedPlaylistId,
  )
  return <PlaylistViewContent key={selectedPlaylistId} onBack={onBack} />
}

function PlaylistViewContent({ onBack }: { onBack?: () => void }) {
  const view = usePlaylistView()
  const [searchOpen, setSearchOpen] = useState(false)
  const searchInputRef = useRef<HTMLInputElement>(null)
  const searchButtonRef = useRef<HTMLButtonElement>(null)
  const { isCompact } = useCompactDisplay()
  const [previousCompact, setPreviousCompact] = useState(isCompact)
  const hasPlayer = usePlayerStore(state => state.currentTrack !== null)
  const hasTracks = view.playlist.tracks.length > 0
  useAndroidBackAction(view.selection.mode, view.selection.exit)

  // Search belongs to the current layout; resizing must not carry a modal query
  // into the desktop playlist, or leave the compact playlist filtered behind it.
  if (previousCompact !== isCompact) {
    setPreviousCompact(isCompact)
    setSearchOpen(false)
    view.search.setValue('')
  }

  const handleSearchOpenChange = (open: boolean) => {
    setSearchOpen(open)
    if (!open) view.search.setValue('')
  }

  const trackTable = (
    <PlaylistTracksTable
      hideHeader={searchOpen}
      {...view.table}
    />
  )

  return (
    <>
      <div className="flex min-h-0 grow flex-col gap-3 md:gap-4 md:pt-4">
        {(hasTracks || onBack) && (
          <PlaylistToolbar
            onBack={onBack}
            currentPlaylist={view.playlist}
            search={view.search.value}
            onSearchChange={view.search.setValue}
            onOpenSearch={() => handleSearchOpenChange(true)}
            searchButtonRef={searchButtonRef}
            actions={view.selection.mode
              ? <PlaylistSelectionActions {...view.bulkActions} />
              : hasTracks ? <PlaylistPlaybackActions {...view.playbackActions} /> : null}
          />
        )}

        {!searchOpen && (hasTracks
          ? (
              trackTable
            )
          : (
              <PlaylistEmptyState
                libraryTrackCount={view.libraryCount}
                playlistId={view.playlist.id}
                isCustom={view.playlist.isCustom}
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
              value={view.search.value}
              onChange={event => view.search.setValue(event.target.value)}
              className="min-w-0 flex-1 border-none bg-transparent text-base shadow-none focus-visible:ring-0 dark:bg-transparent [&::-webkit-search-cancel-button]:hidden"
            />
            {view.search.value && (
              <Button
                variant="ghost"
                size="icon"
                aria-label="Clear track search"
                onClick={() => {
                  view.search.setValue('')
                  searchInputRef.current?.focus()
                }}
              >
                <X className="size-5" />
              </Button>
            )}
          </header>
          <p className="shrink-0 px-5 py-3 text-sm text-muted-foreground" aria-live="polite">
            {view.table.entries.length}
            {' '}
            {view.table.entries.length === 1 ? 'track' : 'tracks'}
            {' '}
            in
            {' '}
            {view.playlist.name}
          </p>
          {view.table.entries.length > 0
            ? trackTable
            : <p className="px-5 py-6 text-sm text-muted-foreground">No tracks found.</p>}
        </DialogContent>
      </Dialog>

      <TrackInfoDialog
        track={view.dialogs.infoTrack}
        open={view.dialogs.infoTrack !== null}
        onOpenChange={view.dialogs.onInfoOpenChange}
      />

      <Dialog
        open={view.dialogs.actionError !== null}
        onOpenChange={view.dialogs.onErrorOpenChange}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Couldn’t finish</DialogTitle>
            <DialogDescription className="whitespace-pre-wrap">
              {view.dialogs.actionError}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="secondary"
              onClick={() => view.dialogs.onErrorOpenChange(false)}
            >
              OK
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
