"use client";

import { useMemo, useRef } from "react";
import { useMutation } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Heart, ListPlus, Music, Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useCachedThumbnail } from "@/hooks/use-cached-thumbnail";
import type { Track } from "@/lib/db";
import {
  ALL_TRACKS_PLAYLIST_ID,
  isTrackLiked,
  resolvePlaylistTracks,
  usePlaylistsStore,
} from "@/stores/playlists-store";
import { useLibraryStore } from "@/stores/library-store";
import { usePlayerStore } from "@/stores/player-store";
import { useTRPC } from "@/trpc/client";

const TRACK_ROW_HEIGHT = 90;

function formatDuration(seconds: number | null): string {
  if (seconds === null) return "--:--";
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

function TrackThumbnail({ trackId }: { trackId: number }) {
  const { url, loaded, failed } = useCachedThumbnail(trackId);

  return (
    <div className="relative size-16 shrink-0 overflow-hidden rounded-lg bg-muted">
      <div className="absolute inset-0 flex items-center justify-center text-muted-foreground">
        <Music className="size-5" />
      </div>
      {!failed && url && (
        <img
          src={url}
          alt="Thumbnail"
          decoding="async"
          className={`absolute inset-0 size-16 object-cover transition-opacity duration-200 ${
            loaded ? "opacity-100" : "opacity-0"
          }`}
        />
      )}
    </div>
  );
}

interface TrackRowProps {
  track: Track;
  isActive: boolean;
  isPlaying: boolean;
  isLiked: boolean;
  customPlaylists: { id: number; name: string; trackIds: number[] }[];
  onTrackSelect: (track: Track) => void;
  onToggleLike: (trackId: number) => void;
  onAddToPlaylist: (playlistId: number, trackId: number) => void;
  isTogglingLike: boolean;
}

function TrackRow({
  track,
  isActive,
  isPlaying,
  isLiked,
  customPlaylists,
  onTrackSelect,
  onToggleLike,
  onAddToPlaylist,
  isTogglingLike,
}: TrackRowProps) {
  const showEqualizer = isActive && isPlaying;
  const availablePlaylists = customPlaylists.filter(
    (playlist) => !playlist.trackIds.includes(track.id),
  );
  const hasCustomPlaylists = customPlaylists.length > 0;

  return (
    <div
      className={`group relative flex items-center gap-3 rounded-xl border px-3 py-2.5 transition-colors duration-200 ${
        isActive
          ? "border-primary/40 bg-primary/10"
          : "border-transparent hover:border-border hover:bg-card/70"
      }`}
      style={{ height: `${TRACK_ROW_HEIGHT}px` }}
    >
      <button
        type="button"
        onClick={() => onTrackSelect(track)}
        aria-label={showEqualizer ? "Pause track" : "Play track"}
        className="relative shrink-0"
      >
        <TrackThumbnail trackId={track.id} />
        <span
          className={`absolute inset-0 flex items-center justify-center rounded-lg bg-background/65 backdrop-blur-[1px] transition-opacity duration-200 ${
            isActive ? "opacity-100" : "opacity-0 group-hover:opacity-100"
          }`}
        >
          {showEqualizer ? (
            <span className="equalizer flex h-4 items-end gap-1">
              <span className="w-[3px] rounded-[1px] bg-foreground" />
              <span />
              <span />
            </span>
          ) : isActive ? (
            <Play className="size-5 fill-primary text-primary" />
          ) : (
            <Play className="size-5 fill-foreground text-foreground" />
          )}
        </span>
      </button>

      <button
        type="button"
        onClick={() => onTrackSelect(track)}
        className="flex min-w-0 grow flex-col items-start text-left"
      >
        <span
          className={`max-w-full truncate text-sm font-medium ${
            isActive ? "text-primary" : "text-foreground"
          }`}
        >
          {track.title ?? "Unknown Title"}
        </span>
        <span className="max-w-full truncate text-sm text-muted-foreground">
          {track.performer ?? "Unknown Artist"}
        </span>
      </button>

      <div className="flex shrink-0 items-center gap-1">
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          onClick={() => onToggleLike(track.id)}
          disabled={isTogglingLike}
          aria-label={isLiked ? "Unlike track" : "Like track"}
          className={
            isLiked
              ? "text-primary hover:text-primary"
              : "text-muted-foreground opacity-0 group-hover:opacity-100"
          }
        >
          <Heart className={isLiked ? "fill-current" : undefined} />
        </Button>

        <TooltipProvider>
          {hasCustomPlaylists ? (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  aria-label="Add to playlist"
                  className="text-muted-foreground opacity-0 group-hover:opacity-100"
                >
                  <ListPlus />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuLabel>Add to playlist</DropdownMenuLabel>
                {availablePlaylists.length === 0 ? (
                  <DropdownMenuItem disabled>
                    Already in all playlists
                  </DropdownMenuItem>
                ) : (
                  availablePlaylists.map((playlist) => (
                    <DropdownMenuItem
                      key={playlist.id}
                      onClick={() => onAddToPlaylist(playlist.id, track.id)}
                    >
                      {playlist.name}
                    </DropdownMenuItem>
                  ))
                )}
              </DropdownMenuContent>
            </DropdownMenu>
          ) : (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  disabled
                  aria-label="Add to playlist"
                  className="text-muted-foreground opacity-0 group-hover:opacity-100"
                >
                  <ListPlus />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                You need to create the playlist first
              </TooltipContent>
            </Tooltip>
          )}
        </TooltipProvider>

        <span className="w-10 shrink-0 text-right font-mono text-xs tabular-nums text-muted-foreground">
          {formatDuration(track.duration)}
        </span>
      </div>
    </div>
  );
}

function getEmptyStateCopy(
  libraryTrackCount: number,
  selectedPlaylistId: typeof ALL_TRACKS_PLAYLIST_ID | number,
  likedPlaylistId: number | undefined,
): { title: string; description: string } {
  if (libraryTrackCount === 0) {
    return {
      title: "No tracks yet",
      description:
        "Pin music to your Telegram profile and it will tune in here automatically.",
    };
  }

  if (selectedPlaylistId === likedPlaylistId) {
    return {
      title: "No liked tracks yet",
      description: "Tap the heart on any track to save it here.",
    };
  }

  if (selectedPlaylistId !== ALL_TRACKS_PLAYLIST_ID) {
    return {
      title: "This playlist is empty",
      description: "Add tracks from your library using the list button.",
    };
  }

  return {
    title: "No tracks yet",
    description:
      "Pin music to your Telegram profile and it will tune in here automatically.",
  };
}

export function TrackList() {
  const trpc = useTRPC();
  const libraryTracks = useLibraryStore((state) => state.tracks);
  const currentTrackId = usePlayerStore(
    (state) => state.currentTrack?.id ?? null,
  );
  const isPlaying = usePlayerStore((state) => state.isPlaying);
  const selectTrack = usePlayerStore((state) => state.selectTrack);
  const data = usePlaylistsStore((state) => state.data);
  const selectedPlaylistId = usePlaylistsStore(
    (state) => state.selectedPlaylistId,
  );
  const activateSelectedPlaylist = usePlaylistsStore(
    (state) => state.activateSelectedPlaylist,
  );
  const setData = usePlaylistsStore((state) => state.setData);
  const scrollRef = useRef<HTMLDivElement>(null);

  const displayTracks = useMemo(
    () => resolvePlaylistTracks(libraryTracks, data, selectedPlaylistId),
    [libraryTracks, data, selectedPlaylistId],
  );

  const toggleLikeMutation = useMutation(
    trpc.playlists.toggleLike.mutationOptions(),
  );
  const addTrackMutation = useMutation(
    trpc.playlists.addTrack.mutationOptions(),
  );

  const virtualizer = useVirtualizer({
    count: displayTracks.length,
    gap: 8,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => TRACK_ROW_HEIGHT,
    overscan: 8,
  });

  const handleTrackSelect = (track: Track) => {
    activateSelectedPlaylist(track.id);
    selectTrack(track);
  };

  const handleToggleLike = async (trackId: number) => {
    if (!data) {
      return;
    }

    try {
      const result = await toggleLikeMutation.mutateAsync({ trackId });
      const nextData = {
        ...data,
        liked: { ...data.liked, trackIds: result.trackIds },
      };
      setData(nextData);
    } catch {
      // keep UI unchanged on failure
    }
  };

  const handleAddToPlaylist = async (playlistId: number, trackId: number) => {
    if (!data) return;

    try {
      await addTrackMutation.mutateAsync({ playlistId, trackId });
      setData({
        ...data,
        custom: data.custom.map((playlist) =>
          playlist.id === playlistId
            ? {
                ...playlist,
                trackIds: playlist.trackIds.includes(trackId)
                  ? playlist.trackIds
                  : [...playlist.trackIds, trackId],
              }
            : playlist,
        ),
      });
    } catch {
      // keep UI unchanged on failure
    }
  };

  if (displayTracks.length === 0) {
    const emptyState = getEmptyStateCopy(
      libraryTracks.length,
      selectedPlaylistId,
      data?.liked.id,
    );

    return (
      <div className="animate-fade-up flex flex-col items-center justify-center gap-3 px-6 py-20 text-center">
        <div className="flex size-14 items-center justify-center rounded-full border border-border bg-card text-muted-foreground">
          <Music className="size-6" />
        </div>
        <p className="text-base font-medium text-foreground">
          {emptyState.title}
        </p>
        <p className="max-w-xs text-sm text-muted-foreground">
          {emptyState.description}
        </p>
      </div>
    );
  }

  const customPlaylists = data?.custom ?? [];

  return (
    <div ref={scrollRef} className="min-h-0 grow overflow-y-auto p-4">
      <ul
        className="relative w-full list-none"
        style={{ height: `${virtualizer.getTotalSize()}px` }}
      >
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const track = displayTracks[virtualRow.index];
          if (!track) {
            return null;
          }

          return (
            <li
              key={track.id}
              className="absolute left-0 top-0 w-full px-0"
              style={{
                height: `${virtualRow.size}px`,
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
              <TrackRow
                track={track}
                isActive={currentTrackId === track.id}
                isPlaying={isPlaying}
                isLiked={isTrackLiked(data, track.id)}
                customPlaylists={customPlaylists}
                onTrackSelect={handleTrackSelect}
                onToggleLike={(trackId) => void handleToggleLike(trackId)}
                onAddToPlaylist={(playlistId, trackId) =>
                  void handleAddToPlaylist(playlistId, trackId)
                }
                isTogglingLike={toggleLikeMutation.isPending}
              />
            </li>
          );
        })}
      </ul>
    </div>
  );
}
