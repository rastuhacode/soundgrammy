"use client";

import { useEffect } from "react";
import type { PlaylistsBundle } from "@/lib/db";
import { useLibraryStore } from "@/stores/library-store";
import { usePlaylistsStore } from "@/stores/playlists-store";

interface PlaylistsHydratorProps {
  playlists: PlaylistsBundle;
  children: React.ReactNode;
}

export function PlaylistsHydrator({
  playlists,
  children,
}: PlaylistsHydratorProps) {
  const hydrate = usePlaylistsStore((state) => state.hydrate);
  const syncQueueToPlayer = usePlaylistsStore((state) => state.syncQueueToPlayer);
  const libraryTracks = useLibraryStore((state) => state.tracks);

  useEffect(() => {
    hydrate(playlists);
  }, [playlists, hydrate]);

  useEffect(() => {
    syncQueueToPlayer();
  }, [libraryTracks, playlists, syncQueueToPlayer]);

  return children;
}
