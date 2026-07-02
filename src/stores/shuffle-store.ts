import { readLocalStorageValue } from '@mantine/hooks'
import { create } from 'zustand'
import type { Track } from '@/lib/db'

import type { ShuffleState, ShuffleAlgorithm } from '@/lib/shuffle'
import { defaultShuffle } from '@/lib/shuffle/default'
import { applyAlgorithm, isShuffleState } from '@/lib/shuffle'

interface ShuffleStoreState {
  shuffle: ShuffleState
  algorithm: ShuffleAlgorithm
  setShuffle: (shuffle: ShuffleState) => void
  setAlgorithm: (algorithm: ShuffleAlgorithm) => void
  toggle: () => void
  process: (
    tracks: Track[],
    pinTrackId?: number,
    algorithm?: ShuffleAlgorithm,
    shuffle?: ShuffleState,
  ) => Track[]
  hydrate: () => void
}

export const useShuffleStore = create<ShuffleStoreState>((set, get) => {
  const { read, write } = useStorageShuffle()

  return {
    shuffle: 'off',
    algorithm: defaultShuffle,
    setShuffle: (shuffle) => {
      write(shuffle)
      set({ shuffle })
    },
    setAlgorithm: algorithm => set({ algorithm }),
    toggle: () => {
      const { shuffle, setShuffle } = get()
      setShuffle(shuffle === 'off' ? 'on' : 'off')
    },
    process: (tracks, pinTrackId, algorithmOverride, shuffleOverride) => {
      const { shuffle, algorithm } = get()
      if ((shuffleOverride ?? shuffle) === 'off') return tracks
      return applyAlgorithm(
        tracks,
        algorithmOverride ?? algorithm,
        pinTrackId,
      )
    },
    hydrate: () => set({ shuffle: read() }),
  }
})

/**
 * Reads and writes the shuffle state to localStorage.
 * @returns The read and write functions.
 */
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
