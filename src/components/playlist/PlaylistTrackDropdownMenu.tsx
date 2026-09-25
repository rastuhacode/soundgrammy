import {
  CheckSquare,
  Download,
  Ellipsis,
  HardDriveDownload,
  HardDriveUpload,
  Heart,
  Info,
  ListEnd,
  ListPlus,
  ListStart,
  ListX,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useCacheStore } from '@/stores/cache-store'
import { cn } from '@/lib/utils'
import {
  getAvailableCustomPlaylists,
  getTrackContextActions,
} from './track-actions'
import type { PlaylistTrackContextMenuProps } from './PlaylistTrackContextMenu'

export function PlaylistTrackDropdownMenu({
  track,
  sourceIndex,
  isLiked,
  currentPlaylist,
  customPlaylists,
  onSelect,
  onToggleLike,
  onAddToPlaylist,
  onDeleteFromPlaylist,
  onPlayNext,
  onAddToEnd,
  onCache,
  onDownload,
  onRemoveFromCache,
  onShowInfo,
}: Omit<PlaylistTrackContextMenuProps, 'children' | 'disabled'>) {
  const actions = getTrackContextActions(currentPlaylist)
  const availablePlaylists = getAvailableCustomPlaylists(customPlaylists)
  const isCached = useCacheStore(state => state.cachedIds.has(track.id))
  const isBusy = useCacheStore(state => state.busyIds.has(track.id))

  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={(
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={`${track.title ?? 'Track'} options`}
          className="touch-visible-option size-9 text-muted-foreground"
          onClick={event => event.stopPropagation()}
          onPointerDown={event => event.stopPropagation()}
        >
          <Ellipsis className="size-4" />
        </Button>
      )}
      />
      <DropdownMenuContent className="w-52" align="end">
        <DropdownMenuGroup>
          <DropdownMenuLabel>Selection</DropdownMenuLabel>
          <DropdownMenuItem onClick={() => onSelect(sourceIndex)}>
            <CheckSquare className="size-4" />
            Select
          </DropdownMenuItem>
        </DropdownMenuGroup>

        <DropdownMenuSeparator />

        <DropdownMenuGroup>
          <DropdownMenuLabel>Queue</DropdownMenuLabel>
          {actions.playNext && (
            <DropdownMenuItem onClick={() => onPlayNext(track)}>
              <ListStart className="size-4" />
              Play next
            </DropdownMenuItem>
          )}
          {actions.addToEnd && (
            <DropdownMenuItem onClick={() => onAddToEnd(track)}>
              <ListEnd className="size-4" />
              Add to end
            </DropdownMenuItem>
          )}
        </DropdownMenuGroup>

        <DropdownMenuSeparator />

        <DropdownMenuGroup>
          <DropdownMenuLabel>Playlist</DropdownMenuLabel>
          {actions.toggleLike && (
            <DropdownMenuItem onClick={() => onToggleLike(track.id)}>
              <Heart className={cn('size-4', isLiked && 'fill-primary text-primary')} />
              {isLiked ? 'Remove from Liked' : 'Add to Liked'}
            </DropdownMenuItem>
          )}
          {actions.addToPlaylist && (
            <DropdownMenuSub>
              <DropdownMenuSubTrigger>
                <ListPlus className="size-4" />
                Add to playlist
              </DropdownMenuSubTrigger>
              <DropdownMenuPortal>
                <DropdownMenuSubContent>
                  {availablePlaylists.length === 0
                    ? (
                        <DropdownMenuItem disabled>
                          <ListPlus className="size-4" />
                          No other playlists
                        </DropdownMenuItem>
                      )
                    : availablePlaylists.map(playlist => (
                        <DropdownMenuItem
                          key={playlist.id}
                          onClick={() => onAddToPlaylist(playlist.id, track.id)}
                        >
                          <ListPlus className="size-4" />
                          {playlist.name}
                        </DropdownMenuItem>
                      ))}
                </DropdownMenuSubContent>
              </DropdownMenuPortal>
            </DropdownMenuSub>
          )}
          {actions.removeFromPlaylist && (
            <DropdownMenuItem onClick={() => onDeleteFromPlaylist(currentPlaylist.id as number, sourceIndex)}>
              <ListX className="size-4" />
              Remove from playlist
            </DropdownMenuItem>
          )}
        </DropdownMenuGroup>

        <DropdownMenuSeparator />

        <DropdownMenuGroup>
          <DropdownMenuLabel>Track</DropdownMenuLabel>
          {actions.cache && !isCached && (
            <DropdownMenuItem disabled={isBusy} onClick={() => onCache(track)}>
              <HardDriveDownload className="size-4" />
              {isBusy ? 'Caching…' : 'Cache'}
            </DropdownMenuItem>
          )}
          {actions.removeFromCache && isCached && (
            <DropdownMenuItem onClick={() => onRemoveFromCache(track)}>
              <HardDriveUpload className="size-4" />
              Remove from cache
            </DropdownMenuItem>
          )}
          {actions.download && (
            <DropdownMenuItem disabled={isBusy} onClick={() => onDownload(track)}>
              <Download className="size-4" />
              {isBusy ? 'Downloading…' : 'Download'}
            </DropdownMenuItem>
          )}
          {actions.showInfo && (
            <DropdownMenuItem onClick={() => onShowInfo(track)}>
              <Info className="size-4" />
              Show info
            </DropdownMenuItem>
          )}
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
