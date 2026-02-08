"use client";

import { useState, useCallback } from "react";
import type { Track } from "../lib/db";
import { TrackList } from "./TrackList";
import { AudioPlayer } from "./AudioPlayer";

interface MusicLibraryProps {
  tracks: Track[];
}

export function MusicLibrary({ tracks }: MusicLibraryProps) {
  const [currentTrack, setCurrentTrack] = useState<Track | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);

  const handleTrackSelect = useCallback(
    (track: Track) => {
      if (currentTrack?.id === track.id) {
        console.log("handleTrackSelect", currentTrack, track);
        setIsPlaying((prev) => !prev);
      } else {
        setCurrentTrack(track);
        setIsPlaying(true);
      }
    },
    [currentTrack],
  );

  const handlePlayingChange = useCallback((playing: boolean) => {
    setIsPlaying(playing);
  }, []);

  const playlist = tracks;
  const handleEnd = () => {
    console.log("handleEnd", currentTrack, playlist);
    if (currentTrack === null || playlist.length === 0) return;

    const index = playlist.indexOf(currentTrack);
    const nextTrack = playlist[index + 1];
    if (nextTrack) {
      setCurrentTrack(nextTrack);
    } else {
      setCurrentTrack(playlist[0]!);
    }
    setIsPlaying(true);
  };

  return (
    <>
      <TrackList
        tracks={tracks}
        currentTrackId={currentTrack?.id ?? null}
        isPlaying={isPlaying}
        onTrackSelect={handleTrackSelect}
      />
      <AudioPlayer
        track={currentTrack}
        isPlaying={isPlaying}
        onPlayingChange={handlePlayingChange}
        onEnd={handleEnd}
      />
    </>
  );
}
