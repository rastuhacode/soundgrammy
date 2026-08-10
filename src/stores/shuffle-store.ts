import { readLocalStorageValue } from '@mantine/hooks'
import { create } from 'zustand'

import type { ShuffleMode, ShuffleState } from '@/lib/shuffle'
import { isShuffleMode, isShuffleState } from '@/lib/shuffle'

interface ShuffleStoreState {
  shuffle: ShuffleState
  mode: ShuffleMode
  setShuffle: (shuffle: ShuffleState) => void
  setMode: (mode: ShuffleMode) => void
  hydrate: () => void
}

export const useShuffleStore = create<ShuffleStoreState>((set) => {
  const storage = useStorageShuffle()

  return {
    shuffle: 'off',
    mode: 'random',
    setShuffle: (shuffle) => {
      storage.writeState(shuffle)
      set({ shuffle })
    },
    setMode: (mode) => {
      storage.writeMode(mode)
      set({ mode })
    },
    hydrate: () => set({
      shuffle: storage.readState(),
      mode: storage.readMode(),
    }),
  }
})

/**
 * Reads and writes the shuffle state to localStorage.
 * @returns The read and write functions.
 */
function useStorageShuffle() {
  const SHUFFLE_STORAGE_KEY = 'soundgrammy-shuffle'
  const SHUFFLE_MODE_STORAGE_KEY = 'soundgrammy-shuffle-mode'

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

  const readState = (): ShuffleState => {
    return readLocalStorageValue<ShuffleState>({
      key: SHUFFLE_STORAGE_KEY,
      defaultValue: 'off',
      deserialize,
    })
  }

  const readMode = (): ShuffleMode => {
    return readLocalStorageValue<ShuffleMode>({
      key: SHUFFLE_MODE_STORAGE_KEY,
      defaultValue: 'random',
      deserialize: (stored) => {
        if (stored === undefined) return 'random'
        try {
          const mode = JSON.parse(stored)
          if (isShuffleMode(mode)) return mode
        }
        catch {
          if (isShuffleMode(stored)) return stored
        }
        return 'random'
      },
    })
  }

  const writeState = (shuffle: ShuffleState) => {
    if (typeof window === 'undefined') return
    localStorage.setItem(SHUFFLE_STORAGE_KEY, JSON.stringify(shuffle))
  }

  const writeMode = (mode: ShuffleMode) => {
    if (typeof window === 'undefined') return
    localStorage.setItem(SHUFFLE_MODE_STORAGE_KEY, JSON.stringify(mode))
  }

  return { readState, readMode, writeState, writeMode }
}
