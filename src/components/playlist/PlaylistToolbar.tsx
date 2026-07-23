import type { ReactNode } from 'react'
import {
  Download,
  HardDriveDownload,
  Loader2,
  Play,
  Search,
  Shuffle,
  Undo2,
  X,
} from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from '@/components/ui/input-group'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import type { ResolvedSelectedPlaylist } from '@/stores/playlists-store'
import { PlaylistBulkActions } from './PlaylistBulkActions'
import { canDownloadPlaylist, type CustomPlaylistRef } from './track-actions'

function ToolbarIconButton({
  label,
  disabled,
  variant = 'secondary',
  onClick,
  children,
}: {
  label: string
  disabled?: boolean
  variant?: 'default' | 'secondary' | 'outline'
  onClick: () => void
  children: ReactNode
}) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={triggerProps => (
          <span className="inline-flex" {...triggerProps}>
            <Button
              size="icon"
              variant={variant}
              disabled={disabled}
              onClick={onClick}
              aria-label={label}
            >
              {children}
            </Button>
          </span>
        )}
      />
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
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
  search: string
  onSearchChange: (value: string) => void
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
  search,
  onSearchChange,
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

  return (
    <TooltipProvider>
      <div className="flex h-fit w-full shrink-0 items-center justify-between gap-4 px-4">
        <div className="grow flex gap-2">
          <ToolbarIconButton label="Play" variant="default" onClick={onPlay}>
            <Play className="size-4" />
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

          <AnimatePresence initial={false}>
            {selectionMode && (
              <motion.div
                key="bulk-actions"
                initial={{ opacity: 0, x: -8 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -8 }}
                transition={{ duration: 0.18, ease: 'easeOut' }}
                className="flex items-center gap-2"
              >
                <Separator orientation="vertical" className="h-full" />

                {selectedTrackIds.length > 0 && (
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
                )}

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

        <div className="min-w-40 max-w-xl w-full px-2">
          <InputGroup>
            <InputGroupInput
              value={search}
              onChange={e => onSearchChange(e.target.value)}
              placeholder="Search tracks"
            />
            <InputGroupAddon>
              <Search className="size-4" />
            </InputGroupAddon>
            {search.length > 0 && (
              <InputGroupButton onClick={() => onSearchChange('')}>
                <X className="size-4" />
              </InputGroupButton>
            )}
          </InputGroup>
        </div>
      </div>
    </TooltipProvider>
  )
}
