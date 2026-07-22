import { Download, HardDriveDownload, Heart, ListEnd, ListPlus, ListStart, ListX } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import type { ResolvedSelectedPlaylist } from '@/stores/playlists-store'
import { cn } from '@/lib/utils'
import {
  getAvailableCustomPlaylists,
  getBulkActions,
  type CustomPlaylistRef,
} from './track-actions'

export interface PlaylistBulkActionsProps {
  selectedTrackIds: number[]
  selectedPositions: number[]
  currentPlaylist: ResolvedSelectedPlaylist
  customPlaylists: CustomPlaylistRef[]
  likedTrackIds: Set<number>
  onAddToLiked: (trackIds: number[]) => void
  onRemoveFromLiked: (trackIds: number[]) => void
  onAddToPlaylist: (playlistId: number, trackIds: number[]) => void
  onRemoveFromPlaylist: (playlistId: number, positions: number[]) => void
  onPlayNext: (trackIds: number[]) => void
  onAddToEnd: (trackIds: number[]) => void
  onCache: (trackIds: number[]) => void
  onDownload: (trackIds: number[]) => void
}

export function PlaylistBulkActions({
  selectedTrackIds,
  selectedPositions,
  currentPlaylist,
  customPlaylists,
  likedTrackIds,
  onAddToLiked,
  onRemoveFromLiked,
  onAddToPlaylist,
  onRemoveFromPlaylist,
  onPlayNext,
  onAddToEnd,
  onCache,
  onDownload,
}: PlaylistBulkActionsProps) {
  const actions = getBulkActions(currentPlaylist)
  const availablePlaylists = getAvailableCustomPlaylists(customPlaylists)
  const unlikedIds = selectedTrackIds.filter(id => !likedTrackIds.has(id))
  const likedIds = selectedTrackIds.filter(id => likedTrackIds.has(id))
  const count = selectedTrackIds.length

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={(
          <Button variant="secondary" className="gap-1.5">
            Actions
            <span className="rounded-md bg-primary/15 px-1.5 py-0.5 font-mono text-[11px] text-primary">
              {count}
            </span>
          </Button>
        )}
      />
      <DropdownMenuContent className="w-52" align="start">
        <DropdownMenuGroup>
          <DropdownMenuLabel>Queue</DropdownMenuLabel>
          {actions.playNext && (
            <DropdownMenuItem onClick={() => onPlayNext(selectedTrackIds)}>
              <ListStart className="size-4" />
              Play next
            </DropdownMenuItem>
          )}
          {actions.addToEnd && (
            <DropdownMenuItem onClick={() => onAddToEnd(selectedTrackIds)}>
              <ListEnd className="size-4" />
              Add to end
            </DropdownMenuItem>
          )}
        </DropdownMenuGroup>

        <DropdownMenuGroup>
          <DropdownMenuLabel>Playlist</DropdownMenuLabel>
          {actions.addToLiked && (
            <DropdownMenuItem
              onClick={() => onAddToLiked(unlikedIds)}
            >
              <Heart className="size-4" />
              Add to Liked
            </DropdownMenuItem>
          )}
          {actions.removeFromLiked && (
            <DropdownMenuItem
              disabled={likedIds.length === 0}
              onClick={() => onRemoveFromLiked(likedIds)}
            >
              <Heart className={cn('size-4 fill-primary text-primary')} />
              Remove from Liked
            </DropdownMenuItem>
          )}

          {actions.addToPlaylist && (
            <DropdownMenuSub>
              <DropdownMenuSubTrigger>
                <ListPlus className="size-4" />
                Add to playlist
              </DropdownMenuSubTrigger>
              <DropdownMenuPortal>
                <DropdownMenuSubContent sideOffset={8}>
                  {availablePlaylists.length === 0
                    ? (
                        <DropdownMenuItem disabled>
                          <ListPlus className="size-4" />
                          No other playlists
                        </DropdownMenuItem>
                      )
                    : (
                        availablePlaylists.map(playlist => (
                          <DropdownMenuItem
                            key={playlist.id}
                            onClick={() =>
                              onAddToPlaylist(playlist.id, selectedTrackIds)}
                          >
                            <ListPlus className="size-4" />
                            {playlist.name}
                          </DropdownMenuItem>
                        ))
                      )}
                </DropdownMenuSubContent>
              </DropdownMenuPortal>
            </DropdownMenuSub>
          )}

          {actions.removeFromPlaylist && (
            <DropdownMenuItem
              onClick={() =>
                onRemoveFromPlaylist(
                  currentPlaylist.id as number,
                  selectedPositions,
                )}
            >
              <ListX className="size-4" />
              Remove from playlist
            </DropdownMenuItem>
          )}
        </DropdownMenuGroup>

        <DropdownMenuGroup>
          <DropdownMenuLabel>Library</DropdownMenuLabel>
          {actions.cache && (
            <DropdownMenuItem onClick={() => onCache(selectedTrackIds)}>
              <HardDriveDownload className="size-4" />
              Cache
            </DropdownMenuItem>
          )}
          {actions.download && (
            <DropdownMenuItem onClick={() => onDownload(selectedTrackIds)}>
              <Download className="size-4" />
              Download
            </DropdownMenuItem>
          )}
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
