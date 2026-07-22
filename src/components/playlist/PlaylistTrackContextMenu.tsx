import {
  CheckSquare,
  Download,
  HardDriveDownload,
  HardDriveUpload,
  Heart,
  Info,
  ListEnd,
  ListPlus,
  ListStart,
  ListX,
} from 'lucide-react'
import type { Track } from '@/lib/db'
import type { ResolvedSelectedPlaylist } from '@/stores/playlists-store'
import { cn } from '@/lib/utils'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuSeparator,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import {
  getAvailableCustomPlaylists,
  getTrackContextActions,
  type CustomPlaylistRef,
} from './track-actions'

export interface PlaylistTrackContextMenuProps {
  track: Track
  sourceIndex: number
  isLiked: boolean
  currentPlaylist: ResolvedSelectedPlaylist
  customPlaylists: CustomPlaylistRef[]
  children: React.ReactNode
  onSelect: (sourceIndex: number) => void
  onToggleLike: (trackId: number) => void
  onAddToPlaylist: (playlistId: number, trackId: number) => void
  onDeleteFromPlaylist: (playlistId: number, position: number) => void
  onPlayNext: (track: Track) => void
  onAddToEnd: (track: Track) => void
  onCache: (track: Track) => void
  onDownload: (track: Track) => void
  onRemoveFromCache: (track: Track) => void
  onShowInfo: (track: Track) => void
}

export function PlaylistTrackContextMenu({
  track,
  sourceIndex,
  isLiked,
  currentPlaylist,
  customPlaylists,
  children,
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
}: PlaylistTrackContextMenuProps) {
  const actions = getTrackContextActions(currentPlaylist)
  const availablePlaylists = getAvailableCustomPlaylists(customPlaylists)

  return (
    <ContextMenu>
      <ContextMenuTrigger className="contents">
        {children}
      </ContextMenuTrigger>
      <ContextMenuContent className="w-52">
        <ContextMenuGroup>
          <ContextMenuLabel>Selection</ContextMenuLabel>
          <ContextMenuItem onClick={() => onSelect(sourceIndex)}>
            <CheckSquare className="size-4" />
            Select
          </ContextMenuItem>
        </ContextMenuGroup>

        <ContextMenuSeparator />

        <ContextMenuGroup>
          <ContextMenuLabel>Queue</ContextMenuLabel>
          {actions.playNext && (
            <ContextMenuItem onClick={() => onPlayNext(track)}>
              <ListStart className="size-4" />
              Play next
            </ContextMenuItem>
          )}
          {actions.addToEnd && (
            <ContextMenuItem onClick={() => onAddToEnd(track)}>
              <ListEnd className="size-4" />
              Add to end
            </ContextMenuItem>
          )}
        </ContextMenuGroup>

        <ContextMenuSeparator />

        <ContextMenuGroup>
          <ContextMenuLabel>Playlist</ContextMenuLabel>
          {actions.toggleLike && (
            <ContextMenuItem onClick={() => onToggleLike(track.id)}>
              <Heart
                className={cn('size-4', isLiked && 'fill-primary text-primary')}
              />
              {isLiked ? 'Remove from Liked' : 'Add to Liked'}
            </ContextMenuItem>
          )}

          {actions.addToPlaylist && (
            <ContextMenuSub>
              <ContextMenuSubTrigger>
                <ListPlus className="size-4" />
                Add to playlist
              </ContextMenuSubTrigger>
              <ContextMenuSubContent>
                {availablePlaylists.length === 0
                  ? (
                      <ContextMenuItem disabled>
                        <ListPlus className="size-4" />
                        No other playlists
                      </ContextMenuItem>
                    )
                  : (
                      availablePlaylists.map(playlist => (
                        <ContextMenuItem
                          key={playlist.id}
                          onClick={() => onAddToPlaylist(playlist.id, track.id)}
                        >
                          <ListPlus className="size-4" />
                          {playlist.name}
                        </ContextMenuItem>
                      ))
                    )}
              </ContextMenuSubContent>
            </ContextMenuSub>
          )}

          {actions.removeFromPlaylist && (
            <ContextMenuItem
              onClick={() =>
                onDeleteFromPlaylist(currentPlaylist.id as number, sourceIndex)}
            >
              <ListX className="size-4" />
              Remove from playlist
            </ContextMenuItem>
          )}
        </ContextMenuGroup>

        <ContextMenuSeparator />

        <ContextMenuGroup>
          <ContextMenuLabel>Track</ContextMenuLabel>
          {actions.cache && (
            <ContextMenuItem onClick={() => onCache(track)}>
              <HardDriveDownload className="size-4" />
              Cache
            </ContextMenuItem>
          )}
          {actions.download && (
            <ContextMenuItem onClick={() => onDownload(track)}>
              <Download className="size-4" />
              Download
            </ContextMenuItem>
          )}
          {actions.removeFromCache && (
            <ContextMenuItem onClick={() => onRemoveFromCache(track)}>
              <HardDriveUpload className="size-4" />
              Remove from cache
            </ContextMenuItem>
          )}
          {actions.showInfo && (
            <ContextMenuItem onClick={() => onShowInfo(track)}>
              <Info className="size-4" />
              Show info
            </ContextMenuItem>
          )}
        </ContextMenuGroup>
      </ContextMenuContent>
    </ContextMenu>
  )
}
