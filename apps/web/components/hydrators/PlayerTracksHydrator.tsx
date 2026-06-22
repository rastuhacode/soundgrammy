"use client";

import { useEffect } from "react";
import type { Track } from "@/lib/db";
import { useLibraryStore } from "@/stores/library-store";
import { usePlaylistsStore } from "@/stores/playlists-store";
import { usePlayerStore } from "@/stores/player-store";

interface PlayerTracksHydratorProps {
  tracks: Track[];
  children: React.ReactNode;
}

export function PlayerTracksHydrator({
  tracks,
  children,
}: PlayerTracksHydratorProps) {
  const setLibraryTracks = useLibraryStore((state) => state.setTracks);
  const syncQueueToPlayer = usePlaylistsStore(
    (state) => state.syncQueueToPlayer,
  );

  useEffect(() => {
    setLibraryTracks(tracks);

    const { currentTrack } = usePlayerStore.getState();
    if (currentTrack) {
      const refreshed =
        tracks.find((track) => track.id === currentTrack.id) ?? null;
      if (refreshed !== currentTrack) {
        usePlayerStore.setState({
          currentTrack: refreshed,
          ...(refreshed ? {} : { isPlaying: false }),
        });
      }
    }

    syncQueueToPlayer();
  }, [tracks, setLibraryTracks, syncQueueToPlayer]);

  return children;
}
