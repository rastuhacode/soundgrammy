import type { ReactNode, Ref } from 'react'
import {
  ArrowLeft,
  Download,
  Ellipsis,
  HardDriveDownload,
  Loader2,
  Play,
  Search,
  Shuffle,
  Undo2,
  X,
} from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import { Button } from '@/components/ui/button'
import { useCompactDisplay } from '@/hooks/use-compact-display'
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from '@/components/ui/input-group'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import type { ResolvedSelectedPlaylist } from '@/stores/playlists-store'
import { PlaylistBulkActions } from './PlaylistBulkActions'
import { canDownloadPlaylist, type CustomPlaylistRef } from '@/lib/playlist-track-actions'

function ToolbarIconButton({
  label,
  disabled,
  variant = 'secondary',
  onClick,
  children,
  className,
}: {
  label: string
  disabled?: boolean
  variant?: 'default' | 'secondary' | 'outline'
  onClick: () => void
  children: ReactNode
  className?: string
}) {
  return (
    <Button
      size="icon"
      variant={variant}
      disabled={disabled}
      onClick={onClick}
      aria-label={label}
      title={label}
      className={className}
    >
      {children}
    </Button>
  )
}

function progressLabel(
  action: string,
  progress: { current: number, total: number } | null,
): string {
  if (progress && progress.total > 0) {
    return `${action} ${progress.current}/${progress.total}…`
  }
  return `${action}…`
}

export interface PlaylistToolbarProps {
  onBack?: () => void
  hasTracks: boolean
  search: string
  onSearchChange: (search: string) => void
  onOpenSearch: () => void
  searchButtonRef?: Ref<HTMLButtonElement>
  selectionMode: boolean
  selectedTrackIds: number[]
  selectedPositions: number[]
  currentPlaylist: ResolvedSelectedPlaylist
  customPlaylists: CustomPlaylistRef[]
  likedTrackIds: Set<number>
  playlistCached: boolean
  playlistDownloading: boolean
  playlistDownloadProgress: { current: number, total: number } | null
  playlistCaching: boolean
  playlistCacheProgress: { current: number, total: number } | null
  onPlay: () => void
  onShuffle: () => void
  onCachePlaylist: () => void
  onDownloadPlaylist: () => void
  onExitSelection: () => void
  onAddToLiked: (trackIds: number[]) => void
  onRemoveFromLiked: (trackIds: number[]) => void
  onAddToPlaylist: (playlistId: number, trackIds: number[]) => void
  onRemoveFromPlaylist: (playlistId: number, positions: number[]) => void
  onPlayNext: () => void
  onAddToEnd: () => void
  onCache: (trackIds: number[]) => void
  onDownload: (trackIds: number[]) => void
}

export function PlaylistToolbar({
  onBack,
  hasTracks,
  search,
  onSearchChange,
  onOpenSearch,
  searchButtonRef,
  selectionMode,
  selectedTrackIds,
  selectedPositions,
  currentPlaylist,
  customPlaylists,
  likedTrackIds,
  playlistCached,
  playlistDownloading,
  playlistDownloadProgress,
  playlistCaching,
  playlistCacheProgress,
  onPlay,
  onShuffle,
  onCachePlaylist,
  onDownloadPlaylist,
  onExitSelection,
  onAddToLiked,
  onRemoveFromLiked,
  onAddToPlaylist,
  onRemoveFromPlaylist,
  onPlayNext,
  onAddToEnd,
  onCache,
  onDownload,
}: PlaylistToolbarProps) {
  const { isCompact } = useCompactDisplay()
  const showDownloadPlaylist = canDownloadPlaylist(currentPlaylist)
  const cacheBusy = playlistCaching
  const downloadBusy = playlistDownloading
  const cacheLabel = cacheBusy
    ? progressLabel('Caching', playlistCacheProgress)
    : playlistCached
      ? 'All tracks cached'
      : 'Cache playlist'
  const downloadLabel = downloadBusy
    ? progressLabel('Downloading', playlistDownloadProgress)
    : 'Download playlist'

  const bulkActions = selectedTrackIds.length > 0 && (
    <PlaylistBulkActions
      selectedTrackIds={selectedTrackIds}
      selectedPositions={selectedPositions}
      currentPlaylist={currentPlaylist}
      customPlaylists={customPlaylists}
      likedTrackIds={likedTrackIds}
      onAddToLiked={onAddToLiked}
      onRemoveFromLiked={onRemoveFromLiked}
      onAddToPlaylist={onAddToPlaylist}
      onRemoveFromPlaylist={onRemoveFromPlaylist}
      onPlayNext={onPlayNext}
      onAddToEnd={onAddToEnd}
      onCache={onCache}
      onDownload={onDownload}
    />
  )

  const searchControl = isCompact
    ? (
        <Button
          ref={searchButtonRef}
          variant="outline"
          size="icon"
          onClick={onOpenSearch}
          aria-label="Search tracks"
          title={search ? `Search: ${search}` : 'Search tracks'}
          className={search ? 'ml-auto text-primary' : 'ml-auto'}
        >
          <Search className="size-5" />
        </Button>
      )
    : (
        <div className="min-w-48 flex-1">
          <InputGroup>
            <InputGroupInput
              type="search"
              aria-label="Search tracks"
              value={search}
              onChange={event => onSearchChange(event.target.value)}
              placeholder="Search tracks"
            />
            <InputGroupAddon>
              <Search className="size-4" />
            </InputGroupAddon>
            {search.length > 0 && (
              <InputGroupButton aria-label="Clear track search" onClick={() => onSearchChange('')}>
                <X className="size-4" />
              </InputGroupButton>
            )}
          </InputGroup>
        </div>
      )

  if (onBack) {
    const trackCount = currentPlaylist.trackIds.length
    return (
      <>
        <header className="flex w-full min-w-0 shrink-0 flex-nowrap items-center gap-1 border-b border-border bg-sidebar/85 px-2 pb-2 pt-[max(0.5rem,env(safe-area-inset-top))] backdrop-blur-sm">
          <Button
            variant="ghost"
            size="icon"
            className="size-10"
            onClick={onBack}
            aria-label="Back to playlists"
            title="Back to playlists"
          >
            <ArrowLeft className="size-5" />
          </Button>
          <div className="min-w-0 flex-1 px-1">
            <h1 className="truncate text-base font-semibold" title={currentPlaylist.name}>
              {currentPlaylist.name}
            </h1>
            <p className="text-xs text-muted-foreground">
              {trackCount}
              {' '}
              {trackCount === 1 ? 'track' : 'tracks'}
            </p>
          </div>

          <div className="compact-playlist-actions flex min-w-43 grow items-center gap-1">
            {selectionMode
              ? (
                  <>
                    {bulkActions}
                    <ToolbarIconButton label="Exit selection" variant="outline" onClick={onExitSelection}>
                      <Undo2 className="size-4" />
                    </ToolbarIconButton>
                  </>
                )
              : hasTracks
                ? (
                    <>
                      <ToolbarIconButton label="Play" variant="default" onClick={onPlay}>
                        <Play className="size-4 text-foreground fill-foreground" />
                      </ToolbarIconButton>
                      <ToolbarIconButton label="Shuffle" onClick={onShuffle} className="hidden min-[480px]:inline-flex">
                        <Shuffle className="size-4" />
                      </ToolbarIconButton>
                      <div className="hidden items-center gap-1 sm:flex">
                        <ToolbarIconButton
                          label={cacheLabel}
                          disabled={playlistCached || cacheBusy || downloadBusy}
                          onClick={onCachePlaylist}
                        >
                          {cacheBusy ? <Loader2 className="size-4 animate-spin" /> : <HardDriveDownload className="size-4" />}
                        </ToolbarIconButton>
                        {showDownloadPlaylist && (
                          <ToolbarIconButton
                            label={downloadLabel}
                            disabled={downloadBusy || cacheBusy}
                            onClick={onDownloadPlaylist}
                          >
                            {downloadBusy ? <Loader2 className="size-4 animate-spin" /> : <Download className="size-4" />}
                          </ToolbarIconButton>
                        )}
                      </div>
                      <DropdownMenu>
                        <DropdownMenuTrigger render={<Button size="icon" variant="ghost" className="sm:hidden" aria-label="More playlist actions"><Ellipsis className="size-5" /></Button>} />
                        <DropdownMenuContent align="end" className="w-48">
                          <DropdownMenuItem onClick={onShuffle} className="min-[480px]:hidden">
                            <Shuffle className="size-4" />
                            Shuffle
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            disabled={playlistCached || cacheBusy || downloadBusy}
                            onClick={onCachePlaylist}
                          >
                            <HardDriveDownload className="size-4" />
                            {cacheLabel}
                          </DropdownMenuItem>
                          {showDownloadPlaylist && (
                            <DropdownMenuItem disabled={downloadBusy || cacheBusy} onClick={onDownloadPlaylist}>
                              <Download className="size-4" />
                              {downloadLabel}
                            </DropdownMenuItem>
                          )}
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </>
                  )
                : null}

            {searchControl}
          </div>
        </header>
      </>
    )
  }

  return (
    <div className="flex h-fit w-full min-w-0 shrink-0 flex-wrap items-center gap-4 px-4">
      <div className="flex max-w-full flex-none flex-wrap items-center gap-2">
        {!selectionMode && (
          <>
            <ToolbarIconButton label="Play" variant="default" onClick={onPlay}>
              <Play className="size-4 text-foreground fill-foreground" />
            </ToolbarIconButton>
            <ToolbarIconButton label="Shuffle" onClick={onShuffle}>
              <Shuffle className="size-4" />
            </ToolbarIconButton>
            <ToolbarIconButton
              label={cacheLabel}
              disabled={playlistCached || cacheBusy || downloadBusy}
              onClick={onCachePlaylist}
            >
              {cacheBusy
                ? <Loader2 className="size-4 animate-spin" />
                : <HardDriveDownload className="size-4" />}
            </ToolbarIconButton>
            {showDownloadPlaylist
              ? (
                  <ToolbarIconButton
                    label={downloadLabel}
                    disabled={downloadBusy || cacheBusy}
                    onClick={onDownloadPlaylist}
                  >
                    {downloadBusy
                      ? <Loader2 className="size-4 animate-spin" />
                      : <Download className="size-4" />}
                  </ToolbarIconButton>
                )
              : null}
          </>
        )}

        <AnimatePresence initial={false}>
          {selectionMode && (
            <motion.div
              key="bulk-actions"
              initial={{ opacity: 0, x: -8 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -8 }}
              transition={{ duration: 0.18, ease: 'easeOut' }}
              className="flex shrink-0 items-center gap-2"
            >
              {bulkActions}

              <ToolbarIconButton
                label="Exit selection"
                variant="outline"
                onClick={onExitSelection}
              >
                <Undo2 className="size-4" />
              </ToolbarIconButton>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {searchControl}
    </div>
  )
}
