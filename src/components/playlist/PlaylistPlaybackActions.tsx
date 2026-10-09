import type { ReactNode } from 'react'
import { Download, Ellipsis, HardDriveDownload, Loader2, Play, Shuffle } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useCompactDisplay } from '@/hooks/use-compact-display'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { canDownloadPlaylist } from '@/lib/playlist-track-actions'
import type { ResolvedSelectedPlaylist } from '@/lib/playlists'

export function PlaylistActionButton({
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

export interface PlaylistPlaybackActionsProps {
  currentPlaylist: ResolvedSelectedPlaylist
  playlistCached: boolean
  playlistDownloading: boolean
  playlistDownloadProgress: { current: number, total: number } | null
  playlistCaching: boolean
  playlistCacheProgress: { current: number, total: number } | null
  onPlay: () => void
  onShuffle: () => void
  onCachePlaylist: () => void
  onDownloadPlaylist: () => void
}

export function PlaylistPlaybackActions({ currentPlaylist, playlistCached, playlistDownloading,
  playlistDownloadProgress, playlistCaching, playlistCacheProgress, onPlay, onShuffle,
  onCachePlaylist, onDownloadPlaylist }: PlaylistPlaybackActionsProps) {
  const { isCompact } = useCompactDisplay()
  const showDownloadPlaylist = canDownloadPlaylist(currentPlaylist)
  const cacheBusy = playlistCaching
  const downloadBusy = playlistDownloading
  const cacheLabel = cacheBusy ? progressLabel('Caching', playlistCacheProgress) : playlistCached ? 'All tracks cached' : 'Cache playlist'
  const downloadLabel = downloadBusy ? progressLabel('Downloading', playlistDownloadProgress) : 'Download playlist'
  return (
    <>
      <PlaylistActionButton label="Play" variant="default" onClick={onPlay}>
        <Play className="size-4 text-foreground fill-foreground" />
      </PlaylistActionButton>
      <PlaylistActionButton label="Shuffle" onClick={onShuffle} className={isCompact ? 'hidden min-[480px]:inline-flex' : undefined}>
        <Shuffle className="size-4" />
      </PlaylistActionButton>
      <div className={isCompact ? 'hidden items-center gap-1 sm:flex' : 'flex items-center gap-2'}>
        <PlaylistActionButton label={cacheLabel} disabled={playlistCached || cacheBusy || downloadBusy} onClick={onCachePlaylist}>
          {cacheBusy ? <Loader2 className="size-4 animate-spin" /> : <HardDriveDownload className="size-4" />}
        </PlaylistActionButton>
        {showDownloadPlaylist && (
          <PlaylistActionButton label={downloadLabel} disabled={downloadBusy || cacheBusy} onClick={onDownloadPlaylist}>
            {downloadBusy ? <Loader2 className="size-4 animate-spin" /> : <Download className="size-4" />}
          </PlaylistActionButton>
        )}
      </div>
      {isCompact && (
        <DropdownMenu>
          <DropdownMenuTrigger render={<Button size="icon" variant="ghost" className="sm:hidden" aria-label="More playlist actions"><Ellipsis className="size-5" /></Button>} />
          <DropdownMenuContent align="end" className="w-48">
            <DropdownMenuItem onClick={onShuffle} className="min-[480px]:hidden">
              <Shuffle className="size-4" />
              Shuffle
            </DropdownMenuItem>
            <DropdownMenuItem disabled={playlistCached || cacheBusy || downloadBusy} onClick={onCachePlaylist}>
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
      )}
    </>
  )
}
