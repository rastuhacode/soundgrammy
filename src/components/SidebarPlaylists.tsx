import { useState } from "react";
import { Ellipsis, Pencil, Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { PlaylistFormDialog } from "@/components/playlist/PlaylistFormDialog";
import { SidebarPlaylistThumbnail } from "@/components/playlist/SidebarPlaylistThumbnail";
import type { CustomPlaylistSummary } from "@/lib/db";
import {
  ALL_TRACKS_PLAYLIST_ID,
  LIKED_PLAYLIST_ID,
  type PlaylistId,
  usePlaylistsStore,
} from "@/stores/playlists-store";
import { useLibraryStore } from "@/stores/library-store";
import { api } from "@/lib/api";
import { cn } from "@/lib/utils";

interface PlaylistItemProps {
  id: PlaylistId;
  name: string;
  count: number;
  isActive: boolean;
  thumbnailVariant:
    | typeof ALL_TRACKS_PLAYLIST_ID
    | typeof LIKED_PLAYLIST_ID
    | "custom";
  playlistId?: number;
  hasThumbnail?: boolean;
  onSelect: () => void;
  onEdit?: () => void;
  onDelete?: () => void;
  isDeleting?: boolean;
}

function PlaylistItem({
  name,
  count,
  isActive,
  thumbnailVariant,
  playlistId,
  hasThumbnail,
  onSelect,
  onEdit,
  onDelete,
  isDeleting,
}: PlaylistItemProps) {
  return (
    <div
      className={cn(
        "group flex w-full items-center gap-3 rounded-lg px-2 py-2 transition-colors",
        isActive
          ? "bg-accent text-accent-foreground"
          : "text-foreground hover:bg-muted/70",
      )}
      role="button"
      aria-label={`Select ${name} playlist`}
      tabIndex={0}
      onClick={onSelect}
    >
      <SidebarPlaylistThumbnail
        variant={thumbnailVariant}
        playlistId={playlistId}
        hasThumbnail={hasThumbnail}
        name={name}
      />

      <div className="min-w-0 grow">
        <p
          className={cn(
            "truncate text-sm font-medium",
            isActive ? "text-foreground" : "text-foreground/90",
          )}
        >
          {name}
        </p>
      </div>

      <div className="flex shrink-0 items-center gap-1">
        <span className="min-w-6 text-right font-mono text-xs tabular-nums text-muted-foreground">
          {count}
        </span>

        <div className="flex size-6 shrink-0 items-center justify-center">
          {onEdit || onDelete
            ? (
                <DropdownMenu>
                  <DropdownMenuTrigger render={(
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-xs"
                      aria-label={`${name} options`}
                      className="text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <Ellipsis />
                    </Button>
                  )}
                  />
                  <DropdownMenuContent align="end" className="w-40">
                    {onEdit
                      ? (
                          <DropdownMenuItem
                            onClick={(event) => {
                              event.stopPropagation();
                              onEdit();
                            }}
                          >
                            <Pencil />
                            Edit playlist
                          </DropdownMenuItem>
                        )
                      : null}
                    {onDelete
                      ? (
                          <DropdownMenuItem
                            variant="destructive"
                            disabled={isDeleting}
                            onClick={(event) => {
                              event.stopPropagation();
                              onDelete();
                            }}
                          >
                            <Trash2 />
                            Delete playlist
                          </DropdownMenuItem>
                        )
                      : null}
                  </DropdownMenuContent>
                </DropdownMenu>
              )
            : null}
        </div>
      </div>
    </div>
  );
}

type DialogState
  = | { mode: "create" }
    | { mode: "edit"; playlist: CustomPlaylistSummary };

export function SidebarPlaylists() {
  const libraryTrackCount = useLibraryStore((state) => state.tracks.length);
  const data = usePlaylistsStore((state) => state.data);
  const selectedPlaylistId = usePlaylistsStore(
    (state) => state.selectedPlaylistId,
  );
  const setSelectedPlaylist = usePlaylistsStore(
    (state) => state.setSelectedPlaylist,
  );
  const setData = usePlaylistsStore((state) => state.setData);

  const [dialogState, setDialogState] = useState<DialogState | null>(null);
  const [deletingId, setDeletingId] = useState<number | null>(null);

  const likedCount = data?.liked.trackIds.length ?? 0;

  const handleDelete = async (id: number) => {
    if (!data) return;
    setDeletingId(id);
    try {
      await api.deletePlaylist(id);
      setData({
        ...data,
        custom: data.custom.filter((playlist) => playlist.id !== id),
      });
    } catch {
      // keep list unchanged on failure
    } finally {
      setDeletingId(null);
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4">
      <div className="flex items-center justify-between gap-2 px-4">
        <h2 className="text-lg font-semibold tracking-tight text-foreground">
          Library
        </h2>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          onClick={() => setDialogState({ mode: "create" })}
          aria-label="Create playlist"
          className="text-muted-foreground hover:text-foreground"
        >
          <Plus />
        </Button>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-2 pb-2">
        <PlaylistItem
          id={ALL_TRACKS_PLAYLIST_ID}
          name="All tracks"
          count={libraryTrackCount}
          isActive={selectedPlaylistId === ALL_TRACKS_PLAYLIST_ID}
          thumbnailVariant={ALL_TRACKS_PLAYLIST_ID}
          onSelect={() => setSelectedPlaylist(ALL_TRACKS_PLAYLIST_ID)}
        />

        {data
          ? (
              <PlaylistItem
                id={LIKED_PLAYLIST_ID}
                name="Liked"
                count={likedCount}
                isActive={selectedPlaylistId === LIKED_PLAYLIST_ID}
                thumbnailVariant={LIKED_PLAYLIST_ID}
                onSelect={() => setSelectedPlaylist(LIKED_PLAYLIST_ID)}
              />
            )
          : null}

        {data?.custom.map((playlist) => (
          <PlaylistItem
            key={playlist.id}
            id={playlist.id}
            name={playlist.name}
            count={playlist.trackIds.length}
            isActive={selectedPlaylistId === playlist.id}
            thumbnailVariant="custom"
            playlistId={playlist.id}
            hasThumbnail={playlist.hasThumbnail}
            onSelect={() => setSelectedPlaylist(playlist.id)}
            onEdit={() => setDialogState({ mode: "edit", playlist })}
            onDelete={() => handleDelete(playlist.id)}
            isDeleting={deletingId === playlist.id}
          />
        ))}
      </div>

      <PlaylistFormDialog
        open={dialogState !== null}
        onOpenChange={(open) => {
          if (!open) setDialogState(null);
        }}
        mode={dialogState?.mode ?? "create"}
        playlist={dialogState?.mode === "edit" ? dialogState.playlist : undefined}
      />
    </div>
  );
}
