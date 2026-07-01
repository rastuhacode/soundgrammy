import { Download, Ellipsis, Heart, Info, ListPlus, ListX } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { Track } from "@/lib/db";
import type { ResolvedSelectedPlaylist } from "@/stores/playlists-store";
import { cn } from "@/lib/utils";

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
  const {
    availablePlaylists,
    currentPlaylist,
    onAddToPlaylist,
    onToggleLike,
    onDeleteFromPlaylist,
    onDownload,
    onShowInfo,
    isLiked,
    track,
  } = props;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={(
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Track options"
            className="text-muted-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
          >
            <Ellipsis className="size-5" />
          </Button>
        )}
        onClick={(e) => e.stopPropagation()}
      />
      <DropdownMenuContent onClick={(e) => e.stopPropagation()}>
        <DropdownMenuItem onClick={() => onToggleLike(track.id)}>
          <Heart
            className={cn("size-4", isLiked && "fill-primary text-primary")}
          />
          {isLiked ? "Remove from Liked" : "Add to Liked"}
        </DropdownMenuItem>

        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuLabel>Add to playlist</DropdownMenuLabel>
          {availablePlaylists.length === 0
            ? (
                <DropdownMenuItem disabled>
                  <ListPlus className="size-4" />
                  No other playlists
                </DropdownMenuItem>
              )
            : (
                availablePlaylists.map((playlist) => (
                  <DropdownMenuItem
                    key={playlist.id}
                    onClick={() => onAddToPlaylist(playlist.id, track.id)}
                  >
                    <ListPlus className="size-4" />
                    {playlist.name}
                  </DropdownMenuItem>
                ))
              )}

          {currentPlaylist.isCustom && (
            <>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onClick={() => onDeleteFromPlaylist(currentPlaylist.id as number, track.id)}
              >
                <ListX className="size-4" />
                Remove from playlist
              </DropdownMenuItem>
            </>
          )}
        </DropdownMenuGroup>

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
