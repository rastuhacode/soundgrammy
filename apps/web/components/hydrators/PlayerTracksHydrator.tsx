"use client";

import { useEffect } from "react";
import type { Track } from "@/lib/db";
import { usePlayerStore } from "@/stores/player-store";

interface PlayerTracksHydratorProps {
  tracks: Track[];
  children: React.ReactNode;
}

export function PlayerTracksHydrator({
  tracks,
  children,
}: PlayerTracksHydratorProps) {
  const setTracks = usePlayerStore((state) => state.setTracks);

  useEffect(() => {
    setTracks(tracks);
  }, [tracks, setTracks]);

  return children;
}
