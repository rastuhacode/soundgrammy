import { create } from "zustand";
import type { Track } from "@/lib/db";
import z from "zod";

export const repeatSchema = z.enum(["none", "one", "all"]);
export type RepeatState = z.infer<typeof repeatSchema>;

export const shuffleSchema = z.enum(["off", "on"]);
export type ShuffleState = z.infer<typeof shuffleSchema>;
export type ShuffleAlgorithm = (playlist: Track[]) => Track[];

export const defaultShuffleAlgorithm: ShuffleAlgorithm = (playlist) => {
  const shuffled = [...playlist];
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [shuffled[i], shuffled[j]] = [shuffled[j]!, shuffled[i]!];
  }
  return shuffled;
};

function applyShuffle(
  orderedTracks: Track[],
  algorithm: ShuffleAlgorithm,
  pinTrackId?: number,
): Track[] {
  if (orderedTracks.length <= 1) {
    return orderedTracks;
  }

  let shuffled = algorithm(orderedTracks);
  if (pinTrackId === undefined) {
    return shuffled;
  }

  const pinIndex = shuffled.findIndex((track) => track.id === pinTrackId);
  if (pinIndex > 0) {
    const [pinned] = shuffled.splice(pinIndex, 1);
    shuffled = [pinned!, ...shuffled];
  }

  return shuffled;
}

function resolvePlaybackTracks(
  orderedTracks: Track[],
  shuffle: ShuffleState,
  algorithm: ShuffleAlgorithm,
  pinTrackId?: number,
): Track[] {
  if (shuffle === "off") {
    return orderedTracks;
  }
  return applyShuffle(orderedTracks, algorithm, pinTrackId);
}

interface PlayerState {
  orderedTracks: Track[];
  tracks: Track[];
  currentTrack: Track | null;
  isPlaying: boolean;
  repeat: RepeatState;
  shuffle: ShuffleState;
  shuffleAlgorithm: ShuffleAlgorithm;

  setQueueTracks: (tracks: Track[]) => void;
  selectTrack: (track: Track) => void;
  setPlaying: (playing: boolean) => void;
  setRepeat: (repeat: RepeatState) => void;
  toggleRepeat: () => void;
  setShuffle: (shuffle: ShuffleState) => void;
  toggleShuffle: () => void;
  playNext: () => void;
  playPrevious: () => void;
}

const repeatCycle: RepeatState[] = ["none", "one", "all"];

export const usePlayerStore = create<PlayerState>((set, get) => ({
  orderedTracks: [],
  tracks: [],
  currentTrack: null,
  isPlaying: false,
  repeat: "none",
  shuffle: "off",
  shuffleAlgorithm: defaultShuffleAlgorithm,

  setQueueTracks: (orderedTracks) => {
    const { shuffle, shuffleAlgorithm, currentTrack } = get();
    set({
      orderedTracks,
      tracks: resolvePlaybackTracks(
        orderedTracks,
        shuffle,
        shuffleAlgorithm,
        currentTrack?.id,
      ),
    });
  },

  selectTrack: (track) => {
    const { currentTrack, isPlaying } = get();
    if (currentTrack?.id === track.id) {
      set({ isPlaying: !isPlaying });
    } else {
      set({ currentTrack: track, isPlaying: true });
    }
  },

  setPlaying: (isPlaying) => set({ isPlaying }),

  setRepeat: (repeat) => set({ repeat }),

  toggleRepeat: () => {
    const { repeat } = get();
    const nextIndex = (repeatCycle.indexOf(repeat) + 1) % repeatCycle.length;
    set({ repeat: repeatCycle[nextIndex]! });
  },

  setShuffle: (shuffle) => {
    const { orderedTracks, shuffleAlgorithm, currentTrack } = get();
    set({
      shuffle,
      tracks: resolvePlaybackTracks(
        orderedTracks,
        shuffle,
        shuffleAlgorithm,
        currentTrack?.id,
      ),
    });
  },

  toggleShuffle: () => {
    const { shuffle } = get();
    get().setShuffle(shuffle === "off" ? "on" : "off");
  },

  playNext: () => {
    const { currentTrack, tracks, repeat } = get();
    if (!currentTrack || tracks.length === 0) return;

    const index = tracks.findIndex((track) => track.id === currentTrack.id);
    if (index === -1) return;

    if (repeat === "one") {
      set({ isPlaying: true });
      return;
    }

    const isLast = index === tracks.length - 1;
    if (isLast && repeat === "none") {
      set({ isPlaying: false });
      return;
    }

    const nextTrack = tracks[isLast ? 0 : index + 1]!;
    set({ currentTrack: nextTrack, isPlaying: true });
  },

  playPrevious: () => {
    const { currentTrack, tracks, repeat } = get();
    if (!currentTrack || tracks.length === 0) return;

    const index = tracks.findIndex((track) => track.id === currentTrack.id);
    if (index === -1) return;

    if (index === 0 && repeat === "none") {
      return;
    }

    const previousTrack = tracks[index - 1] ?? tracks[tracks.length - 1]!;
    set({ currentTrack: previousTrack, isPlaying: true });
  },
}));
