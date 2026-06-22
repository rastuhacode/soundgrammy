"use client";

import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { Check, Heart, ListMusic, Plus, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  ALL_TRACKS_PLAYLIST_ID,
  usePlaylistsStore,
} from "@/stores/playlists-store";
import { useLibraryStore } from "@/stores/library-store";
import { useTRPC } from "@/trpc/client";

function PlaylistCount({ count }: { count: number }) {
  return (
    <span className="shrink-0 font-mono text-xs tabular-nums text-muted-foreground">
      {count}
    </span>
  );
}

interface PlaylistItemProps {
  id: typeof ALL_TRACKS_PLAYLIST_ID | number;
  name: string;
  count: number;
  isActive: boolean;
  icon: React.ReactNode;
  onSelect: () => void;
  onDelete?: () => void;
  isDeleting?: boolean;
}

function PlaylistItem({
  name,
  count,
  isActive,
  icon,
  onSelect,
  onDelete,
  isDeleting,
}: PlaylistItemProps) {
  return (
    <div
      className={`group flex items-center gap-2 w-full justify-between rounded-xl border px-3 py-2 transition-colors cursor-pointer ${
        isActive
          ? "border-primary/40 bg-primary/10"
          : "border-transparent hover:border-border hover:bg-card/70"
      }`}
      role="button"
      onClick={onSelect}
    >
      <div className="flex items-center gap-2">
        <span
          className={`shrink-0 ${isActive ? "text-primary" : "text-muted-foreground"}`}
        >
          {icon}
        </span>
        <span
          className={`min-w-0 truncate text-sm font-medium ${
            isActive ? "text-primary" : "text-foreground"
          }`}
        >
          {name}
        </span>
        <PlaylistCount count={count} />
      </div>

      {onDelete ? (
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          onClick={onDelete}
          disabled={isDeleting}
          aria-label={`Delete ${name}`}
          className="shrink-0 text-muted-foreground opacity-0 transition-opacity hover:text-destructive group-hover:opacity-100"
        >
          <Trash2 />
        </Button>
      ) : null}
    </div>
  );
}

export function SidebarPlaylists() {
  const trpc = useTRPC();
  const libraryTrackCount = useLibraryStore((state) => state.tracks.length);
  const data = usePlaylistsStore((state) => state.data);
  const selectedPlaylistId = usePlaylistsStore(
    (state) => state.selectedPlaylistId,
  );
  const setSelectedPlaylist = usePlaylistsStore(
    (state) => state.setSelectedPlaylist,
  );
  const setData = usePlaylistsStore((state) => state.setData);

  const [isCreating, setIsCreating] = useState(false);
  const [newPlaylistName, setNewPlaylistName] = useState("");
  const [deletingId, setDeletingId] = useState<number | null>(null);

  const createMutation = useMutation(trpc.playlists.create.mutationOptions());
  const deleteMutation = useMutation(trpc.playlists.delete.mutationOptions());

  const likedCount = data?.liked.trackIds.length ?? 0;

  const handleCreate = async () => {
    const name = newPlaylistName.trim();
    if (!name || !data) {
      return;
    }

    try {
      const created = await createMutation.mutateAsync({ name });
      setData({
        ...data,
        custom: [...data.custom, created],
      });
      setNewPlaylistName("");
      setIsCreating(false);
    } catch {
      // mutation error surfaced by react-query if needed
    }
  };

  const handleDelete = async (id: number) => {
    if (!data) {
      return;
    }

    setDeletingId(id);
    try {
      await deleteMutation.mutateAsync({ id });
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

  const cancelCreate = () => {
    setIsCreating(false);
    setNewPlaylistName("");
  };

  return (
    <div className="flex min-h-0 flex-col gap-2 px-4">
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-lg font-semibold tracking-tight text-foreground">
          Library
        </h2>
        {!isCreating ? (
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={() => setIsCreating(true)}
            aria-label="Create playlist"
          >
            <Plus />
          </Button>
        ) : null}
      </div>

      {isCreating ? (
        <div className="flex items-center gap-1 rounded-xl border border-border bg-card/70 px-2 py-1.5">
          <Input
            value={newPlaylistName}
            onChange={(event) => setNewPlaylistName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                void handleCreate();
              }
              if (event.key === "Escape") {
                cancelCreate();
              }
            }}
            placeholder="Playlist name"
            autoFocus
            className="h-8 border-0 bg-transparent px-1 shadow-none focus-visible:ring-0"
          />
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={() => void handleCreate()}
            disabled={!newPlaylistName.trim() || createMutation.isPending}
            aria-label="Create playlist"
          >
            <Check />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={cancelCreate}
            aria-label="Cancel"
          >
            <X />
          </Button>
        </div>
      ) : null}

      <div className="flex flex-col gap-1">
        <PlaylistItem
          id={ALL_TRACKS_PLAYLIST_ID}
          name="All tracks"
          count={libraryTrackCount}
          isActive={selectedPlaylistId === ALL_TRACKS_PLAYLIST_ID}
          icon={<ListMusic className="size-4" />}
          onSelect={() => setSelectedPlaylist(ALL_TRACKS_PLAYLIST_ID)}
        />

        {data ? (
          <PlaylistItem
            id={data.liked.id}
            name="Liked"
            count={likedCount}
            isActive={selectedPlaylistId === data.liked.id}
            icon={<Heart className="size-4" />}
            onSelect={() => setSelectedPlaylist(data.liked.id)}
          />
        ) : null}

        {data?.custom.map((playlist) => (
          <PlaylistItem
            key={playlist.id}
            id={playlist.id}
            name={playlist.name}
            count={playlist.trackIds.length}
            isActive={selectedPlaylistId === playlist.id}
            icon={<ListMusic className="size-4" />}
            onSelect={() => setSelectedPlaylist(playlist.id)}
            onDelete={() => void handleDelete(playlist.id)}
            isDeleting={deletingId === playlist.id}
          />
        ))}
      </div>
    </div>
  );
}
