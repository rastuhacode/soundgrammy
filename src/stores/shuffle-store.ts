import { readLocalStorageValue } from "@mantine/hooks";
import { create } from "zustand";
import type { Track } from "@/lib/db";
import { z } from "zod";

const SHUFFLE_STORAGE_KEY = "soundgrammy-shuffle";

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

function parseStoredShuffle(stored: string | undefined): ShuffleState {
  if (stored === undefined) return "off";
  try {
    const result = shuffleSchema.safeParse(JSON.parse(stored));
    if (result.success) return result.data;
  } catch {
    const result = shuffleSchema.safeParse(stored);
    if (result.success) return result.data;
  }
  return "off";
}

function persistShuffle(shuffle: ShuffleState) {
  if (typeof window === "undefined") return;
  localStorage.setItem(SHUFFLE_STORAGE_KEY, JSON.stringify(shuffle));
}

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

interface ShuffleStoreState {
  shuffle: ShuffleState;
  shuffleAlgorithm: ShuffleAlgorithm;
  setShuffle: (shuffle: ShuffleState) => void;
  toggleShuffle: () => void;
  resolvePlaybackTracks: (
    orderedTracks: Track[],
    pinTrackId?: number,
  ) => Track[];
  hydrate: () => void;
}

export const useShuffleStore = create<ShuffleStoreState>((set, get) => ({
  shuffle: "off",
  shuffleAlgorithm: defaultShuffleAlgorithm,

  setShuffle: (shuffle) => {
    persistShuffle(shuffle);
    set({ shuffle });
  },

  toggleShuffle: () => {
    const { shuffle } = get();
    get().setShuffle(shuffle === "off" ? "on" : "off");
  },

  resolvePlaybackTracks: (orderedTracks, pinTrackId) => {
    const { shuffle, shuffleAlgorithm } = get();
    if (shuffle === "off") {
      return orderedTracks;
    }
    return applyShuffle(orderedTracks, shuffleAlgorithm, pinTrackId);
  },

  hydrate: () => {
    const shuffle = readLocalStorageValue<ShuffleState>({
      key: SHUFFLE_STORAGE_KEY,
      defaultValue: "off",
      deserialize: parseStoredShuffle,
    });
    set({ shuffle });
  },
}));
