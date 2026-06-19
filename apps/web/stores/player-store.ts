import { create } from "zustand";
import type { Track } from "@/lib/db";

interface PlayerState {
  tracks: Track[];
  currentTrack: Track | null;
  isPlaying: boolean;
  setTracks: (tracks: Track[]) => void;
  selectTrack: (track: Track) => void;
  setPlaying: (playing: boolean) => void;
  playNext: () => void;
}

export const usePlayerStore = create<PlayerState>((set, get) => ({
  tracks: [],
  currentTrack: null,
  isPlaying: false,

  setTracks: (tracks) =>
    set((state) => ({
      tracks,
      currentTrack: state.currentTrack
        ? (tracks.find((track) => track.id === state.currentTrack!.id) ??
          null)
        : null,
    })),

  selectTrack: (track) => {
    const { currentTrack, isPlaying } = get();
    if (currentTrack?.id === track.id) {
      set({ isPlaying: !isPlaying });
    } else {
      set({ currentTrack: track, isPlaying: true });
    }
  },

  setPlaying: (isPlaying) => set({ isPlaying }),

  playNext: () => {
    const { currentTrack, tracks } = get();
    if (!currentTrack || tracks.length === 0) return;

    const index = tracks.findIndex((track) => track.id === currentTrack.id);
    if (index === -1) return;

    const nextTrack = tracks[index + 1] ?? tracks[0]!;
    set({ currentTrack: nextTrack, isPlaying: true });
  },
}));
