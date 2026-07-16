import {
  CheckSquare,
  Download,
  Heart,
  Info,
  ListPlus,
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
  isLiked: boolean
  currentPlaylist: ResolvedSelectedPlaylist
  customPlaylists: CustomPlaylistRef[]
  children: React.ReactNode
  onSelect: (trackId: number) => void
  onToggleLike: (trackId: number) => void
  onAddToPlaylist: (playlistId: number, trackId: number) => void
  onDeleteFromPlaylist: (playlistId: number, trackId: number) => void
  onDownload: (track: Track) => void
  onShowInfo: (track: Track) => void
}

export function PlaylistTrackContextMenu({
  track,
  isLiked,
  currentPlaylist,
  customPlaylists,
  children,
  onSelect,
  onToggleLike,
  onAddToPlaylist,
  onDeleteFromPlaylist,
  onDownload,
  onShowInfo,
}: PlaylistTrackContextMenuProps) {
  const actions = getTrackContextActions(currentPlaylist)
  const availablePlaylists = getAvailableCustomPlaylists(customPlaylists, [
    track.id,
  ])

  return (
    <ContextMenu>
      <ContextMenuTrigger className="contents">
        {children}
      </ContextMenuTrigger>
      <ContextMenuContent className="w-48">
        <ContextMenuGroup>
          <ContextMenuLabel>Selection</ContextMenuLabel>
          <ContextMenuItem onClick={() => onSelect(track.id)}>
            <CheckSquare className="size-4" />
            Select
          </ContextMenuItem>
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
                onDeleteFromPlaylist(currentPlaylist.id as number, track.id)}
            >
              <ListX className="size-4" />
              Remove from playlist
            </ContextMenuItem>
          )}
        </ContextMenuGroup>

        <ContextMenuSeparator />

        <ContextMenuGroup>
          <ContextMenuLabel>Track</ContextMenuLabel>
          {actions.download && (
            <ContextMenuItem onClick={() => onDownload(track)}>
              <Download className="size-4" />
              Download
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
