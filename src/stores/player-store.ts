import { create } from 'zustand'
import type { Track } from '@/lib/db'
import { useRepeatStore } from '@/stores/repeat-store'
import { useShuffleStore, type ShuffleState } from '@/stores/shuffle-store'

interface PlayerState {
  orderedTracks: Track[]
  tracks: Track[]
  currentTrack: Track | null
  isPlaying: boolean

  setQueueTracks: (tracks: Track[]) => void
  selectTrack: (track: Track) => void
  setPlaying: (playing: boolean) => void
  setShuffle: (shuffle: ShuffleState) => void
  toggleShuffle: () => void
  hydratePreferences: () => void
  refreshPlaybackTracks: () => void
  playNext: () => void
  playPrevious: () => void
}

export const usePlayerStore = create<PlayerState>((set, get) => ({
  orderedTracks: [],
  tracks: [],
  currentTrack: null,
  isPlaying: false,

  setQueueTracks: (orderedTracks) => {
    const { currentTrack } = get()
    set({
      orderedTracks,
      tracks: useShuffleStore
        .getState()
        .resolvePlaybackTracks(orderedTracks, currentTrack?.id),
    })
  },

  selectTrack: (track) => {
    const { currentTrack, isPlaying } = get()
    if (currentTrack?.id === track.id) {
      set({ isPlaying: !isPlaying })
    }
    else {
      set({ currentTrack: track, isPlaying: true })
    }
  },

  setPlaying: isPlaying => set({ isPlaying }),

  setShuffle: (shuffle) => {
    useShuffleStore.getState().setShuffle(shuffle)
    get().refreshPlaybackTracks()
  },

  toggleShuffle: () => {
    useShuffleStore.getState().toggleShuffle()
    get().refreshPlaybackTracks()
  },

  refreshPlaybackTracks: () => {
    const { orderedTracks, currentTrack } = get()
    set({
      tracks: useShuffleStore
        .getState()
        .resolvePlaybackTracks(orderedTracks, currentTrack?.id),
    })
  },

  hydratePreferences: () => {
    useRepeatStore.getState().hydrate()
    useShuffleStore.getState().hydrate()
    get().refreshPlaybackTracks()
  },

  playNext: () => {
    const { currentTrack, tracks } = get()
    const { repeat } = useRepeatStore.getState()
    if (!currentTrack || tracks.length === 0) return

    const index = tracks.findIndex(track => track.id === currentTrack.id)
    if (index === -1) return

    if (repeat === 'one') {
      set({ isPlaying: true })
      return
    }

    const isLast = index === tracks.length - 1
    if (isLast && repeat === 'none') {
      set({ isPlaying: false })
      return
    }

    const nextTrack = tracks[isLast ? 0 : index + 1]!
    set({ currentTrack: nextTrack, isPlaying: true })
  },

  playPrevious: () => {
    const { currentTrack, tracks } = get()
    const { repeat } = useRepeatStore.getState()
    if (!currentTrack || tracks.length === 0) return

    const index = tracks.findIndex(track => track.id === currentTrack.id)
    if (index === -1) return

    if (index === 0 && repeat === 'none') {
      return
    }

    const previousTrack = tracks[index - 1] ?? tracks[tracks.length - 1]!
    set({ currentTrack: previousTrack, isPlaying: true })
  },
}))
