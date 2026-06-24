"use client";

import type { Track } from "@/lib/db";
import { useLibraryStore } from "@/stores/library-store";
import { usePlayerStore } from "@/stores/player-store";
import { isTrackLiked, LIKED_PLAYLIST_ID, resolveSelectedPlaylistTracks, usePlaylistsStore } from "@/stores/playlists-store";
import type { CustomPlaylistId, ResolvedSelectedPlaylist } from "@/stores/playlists-store";
import { useTRPC } from "@/trpc/client";
import { useMutation } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Music } from "lucide-react";
import { useMemo, useRef, useState } from "react";
import { Separator } from "@/components/ui/separator";

import { TRACK_ROW_HEIGHT, PlaylistTrackRow } from "./PlaylistTrackRow";
import { TrackInfoDialog } from "./TrackInfoDialog";

function getEmptyStateCopy(
  libraryTrackCount: number,
  playlistId: ResolvedSelectedPlaylist["id"],
  isCustom: boolean,
): { title: string; description: string } {
  if (libraryTrackCount === 0) {
    return {
      title: "No tracks yet",
      description:
          "Pin music to your Telegram profile and it will tune in here automatically.",
    };
  }

  if (playlistId === LIKED_PLAYLIST_ID) {
    return {
      title: "No liked tracks yet",
      description: "Tap the heart on any track to save it here.",
    };
  }

  if (isCustom) {
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

export function PlaylistView() {
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
  const [infoTrack, setInfoTrack] = useState<Track | null>(null);

  const selectedPlaylist = useMemo(
    () =>
      resolveSelectedPlaylistTracks(libraryTracks, data, selectedPlaylistId),
    [libraryTracks, data, selectedPlaylistId],
  );
  const { tracks, isCustom, name: playlistName, id: playlistId } = selectedPlaylist;

  const toggleLikeMutation = useMutation(
    trpc.playlists.toggleLike.mutationOptions(),
  );
  const addTrackMutation = useMutation(
    trpc.playlists.addTrack.mutationOptions(),
  );
  const deleteFromPlaylistMutation = useMutation(
    trpc.playlists.removeTrack.mutationOptions(),
  );

  const virtualizer = useVirtualizer({
    count: tracks.length,
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
    if (!data) return;

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

  const handleDeleteFromPlaylist = async (
    playlistId: CustomPlaylistId,
    trackId: number,
  ) => {
    if (!data) return;

    try {
      await deleteFromPlaylistMutation.mutateAsync({ playlistId, trackId });
      setData({
        ...data,
        custom: data.custom.map((playlist) =>
          playlist.id === playlistId
            ? { ...playlist, trackIds: playlist.trackIds.filter((id) => id !== trackId) }
            : playlist,
        ),
      });
    } catch {
      // keep UI unchanged on failure
    }
  };

  const handleDownload = (track: Track) => {
    const link = document.createElement("a");
    link.href = `/api/tracks/${track.id}/download`;
    link.rel = "noopener";
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  const handleShowInfo = (track: Track) => {
    setInfoTrack(track);
  };

  if (tracks.length === 0) {
    const emptyState = getEmptyStateCopy(
      libraryTracks.length,
      playlistId,
      isCustom,
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
    <>
      <div className="flex min-h-0 grow flex-col pt-4">
        <div className="h-10 px-4 shrink-0 flex items-center">
          <h2 className="text-lg font-semibold">{playlistName}</h2>
        </div>

        <Separator className="mt-4" />

        <div ref={scrollRef} className="min-h-0 grow overflow-y-auto p-4">
          <ul
            className="relative w-full list-none"
            style={{ height: `${virtualizer.getTotalSize()}px` }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const track = tracks[virtualRow.index];
              if (!track) return null;

              return (
                // <li
                //   key={track.id}
                //   className="absolute left-0 top-0 w-full px-0"
                //   style={{ height: `${virtualRow.size}px`, transform: `translateY(${virtualRow.start}px)` }}
                // >
                <PlaylistTrackRow
                  key={track.id}
                  className="absolute left-0 top-0 w-full"
                  style={{ height: `${virtualRow.size}px`, transform: `translateY(${virtualRow.start}px)` }}

                  currentPlaylist={selectedPlaylist}
                  track={track}
                  isActive={currentTrackId === track.id}
                  isPlaying={isPlaying}
                  isLiked={isTrackLiked(data, track.id)}
                  customPlaylists={customPlaylists}
                  onTrackSelect={handleTrackSelect}
                  onToggleLike={handleToggleLike}
                  onAddToPlaylist={handleAddToPlaylist}
                  onDeleteFromPlaylist={handleDeleteFromPlaylist}
                  onDownload={handleDownload}
                  onShowInfo={handleShowInfo}
                />
                // </li>
              );
            })}
          </ul>
        </div>
      </div>

      <TrackInfoDialog
        track={infoTrack}
        open={infoTrack !== null}
        onOpenChange={(open) => {
          if (!open) {
            setInfoTrack(null);
          }
        }}
      />
    </>
  );
}
