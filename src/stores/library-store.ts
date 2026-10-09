import { create } from 'zustand'
import type { Track } from '@/types'
import { api } from '@/lib/api'
import { errorMessage } from '@/lib/errors'
import { captureSession, isSessionCurrent } from '@/stores/session-store'
import { usePlayerStore } from '@/stores/player-store'

interface LibraryState {
  library: Track[]
  error: string | null
  revision: number
  setLibrary: (tracks: Track[]) => void
  refresh: () => Promise<void>
}

export const useLibraryStore = create<LibraryState>((set, get) => ({
  library: [],
  error: null,
  revision: 0,
  setLibrary: library => set(state => ({ library, error: null, revision: state.revision + 1 })),
  refresh: async () => {
    const generation = captureSession()
    const revision = get().revision + 1
    set({ revision })
    const current = () => isSessionCurrent(generation) && get().revision === revision
    try {
      const tracks = await api.listTracks()
      if (!current()) return
      set({ library: tracks, error: null })
      usePlayerStore.getState().refreshQueueTracks(tracks)
    }
    catch (error) {
      if (current()) set({ error: errorMessage(error) })
    }
  },
}))
