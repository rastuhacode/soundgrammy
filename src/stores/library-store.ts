import { create } from "zustand";
import type { Track } from "@/lib/db";

interface LibraryState {
  tracks: Track[];
  setTracks: (tracks: Track[]) => void;
}

export const useLibraryStore = create<LibraryState>((set) => ({
  tracks: [],
  setTracks: (tracks) => set({ tracks }),
}));
