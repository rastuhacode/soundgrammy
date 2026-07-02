import { create } from 'zustand'
import type { Track } from '@/lib/db'

interface LibraryState {
  library: Track[]
  setLibrary: (tracks: Track[]) => void
}

export const useLibraryStore = create<LibraryState>(set => ({
  library: [],
  setLibrary: library => set({ library }),
}))
