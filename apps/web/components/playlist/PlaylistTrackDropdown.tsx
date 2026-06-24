import { Download, Ellipsis, Heart, Info, ListPlus, ListX } from "lucide-react";
import { Button } from "../ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuSubContent, DropdownMenuGroup, DropdownMenuPortal, DropdownMenuSub, DropdownMenuSubTrigger, DropdownMenuTrigger, DropdownMenuItem, DropdownMenuSeparator } from "../ui/dropdown-menu";
import type { Track } from "@/lib/db";
import type { ResolvedSelectedPlaylist } from "@/stores/playlists-store";

export interface PlaylistTrackDropdownProps {
  availablePlaylists: { id: number; name: string; trackIds: number[] }[];
  currentPlaylist: ResolvedSelectedPlaylist;
  onAddToPlaylist: (playlistId: number, trackId: number) => void;
  onToggleLike: (trackId: number) => void;
  onDeleteFromPlaylist: (playlistId: number, trackId: number) => void;
  onDownload: (track: Track) => void;
  onShowInfo: (track: Track) => void;
  isLiked: boolean;
  track: Track;
}

export function PlaylistTrackDropdown(props: PlaylistTrackDropdownProps) {
  const { availablePlaylists, currentPlaylist, onAddToPlaylist, onToggleLike, onDeleteFromPlaylist, onDownload, onShowInfo, isLiked, track } = props;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={(
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Add to playlist"
            className="text-muted-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
          >
            <Ellipsis className="size-5" />
          </Button>
        )}
        onClick={(e) => e.stopPropagation()}
      />
      <DropdownMenuContent className="w-56" onClick={(e) => e.stopPropagation()}>
        <DropdownMenuGroup>
          <DropdownMenuSub>
            <DropdownMenuSubTrigger disabled={availablePlaylists.length === 0}>
              <ListPlus className="size-4" />
              Add to playlist
            </DropdownMenuSubTrigger>
            <DropdownMenuPortal>
              <DropdownMenuSubContent sideOffset={10}>
                {availablePlaylists.map((playlist) => (
                  <DropdownMenuItem
                    key={playlist.id}
                    onClick={() => onAddToPlaylist(playlist.id, track.id)}
                  >
                    {playlist.name}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuSubContent>
            </DropdownMenuPortal>
          </DropdownMenuSub>
          <DropdownMenuItem onClick={() => onToggleLike(track.id)}>
            <Heart className={"size-4 " + (isLiked && "fill-primary text-primary")} />
            { isLiked ? "Remove from Liked" : "Add to Liked"}
          </DropdownMenuItem>
        </DropdownMenuGroup>

        {currentPlaylist.isCustom && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => onDeleteFromPlaylist(currentPlaylist.id, track.id)}>
              <ListX className="size-4" />
              Remove from playlist
            </DropdownMenuItem>
          </>
        )}

        <DropdownMenuSeparator />
        <DropdownMenuItem onClick={() => onDownload(track)}>
          <Download className="size-4" />
          Download
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => onShowInfo(track)}>
          <Info className="size-4" />
          Show info
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
