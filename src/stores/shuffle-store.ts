import { readLocalStorageValue } from '@mantine/hooks'
import { create } from 'zustand'
import type { Track } from '@/lib/db'

import type { ShuffleState, ShuffleAlgorithm } from '@/lib/shuffle'
import { defaultShuffle } from '@/lib/shuffle/default'
import { isShuffleState } from '@/lib/shuffle'

function useStorageShuffle() {
  const SHUFFLE_STORAGE_KEY = 'soundgrammy-shuffle'

  function deserialize(stored: string | undefined): ShuffleState {
    if (stored === undefined) return 'off'
    try {
      const state = JSON.parse(stored)
      if (isShuffleState(state)) return state
    }
    catch {
      if (isShuffleState(stored)) return stored
    }
    return 'off'
  }

  const read = (): ShuffleState => {
    return readLocalStorageValue<ShuffleState>({
      key: SHUFFLE_STORAGE_KEY,
      defaultValue: 'off',
      deserialize,
    })
  }

  const write = (shuffle: ShuffleState) => {
    if (typeof window === 'undefined') return
    localStorage.setItem(SHUFFLE_STORAGE_KEY, JSON.stringify(shuffle))
  }

  return { read, write }
}

function applyShuffle(
  orderedTracks: Track[],
  algorithm: ShuffleAlgorithm,
  pinTrackId?: number,
): Track[] {
  if (orderedTracks.length <= 1) return orderedTracks

  let shuffled = algorithm(orderedTracks)
  if (pinTrackId === undefined) return shuffled

  const pinIndex = shuffled.findIndex(track => track.id === pinTrackId)
  if (pinIndex > 0) {
    const [pinned] = shuffled.splice(pinIndex, 1)
    shuffled = [pinned!, ...shuffled]
  }

  return shuffled
}

interface ShuffleStoreState {
  shuffle: ShuffleState
  shuffleAlgorithm: ShuffleAlgorithm
  setShuffle: (shuffle: ShuffleState) => void
  toggleShuffle: () => void
  resolvePlaybackTracks: (
    orderedTracks: Track[],
    pinTrackId?: number,
  ) => Track[]
  hydrate: () => void
}

export const useShuffleStore = create<ShuffleStoreState>((set, get) => {
  const { read, write } = useStorageShuffle()

  return {
    shuffle: 'off',
    shuffleAlgorithm: defaultShuffle,
    setShuffle: (shuffle) => {
      write(shuffle)
      set({ shuffle })
    },
    toggleShuffle: () => {
      const { shuffle, setShuffle } = get()
      setShuffle(shuffle === 'off' ? 'on' : 'off')
    },
    resolvePlaybackTracks: (orderedTracks, pinTrackId) => {
      const { shuffle, shuffleAlgorithm } = get()
      if (shuffle === 'off') return orderedTracks
      return applyShuffle(orderedTracks, shuffleAlgorithm, pinTrackId)
    },
    hydrate: () => set({ shuffle: read() }),
  }
})
