import { readLocalStorageValue } from '@mantine/hooks'
import { create } from 'zustand'
import { isShuffleMode, isShuffleState, type ShuffleMode, type ShuffleState } from '@/lib/shuffle'

/** Read-only UI mirror; native playback owns both the mode and queue order. */
export const useShuffleStore = create<{ shuffle: ShuffleState, mode: ShuffleMode }>(() => ({ shuffle: 'off', mode: 'random' }))

function legacyValue(key: string): unknown {
  return readLocalStorageValue<unknown>({
    key, defaultValue: null,
    deserialize: (stored) => {
      try {
        return JSON.parse(stored ?? 'null')
      }
      catch { return stored }
    },
  })
}
export function readLegacyShuffle(): { shuffle: ShuffleState, mode: ShuffleMode } {
  const shuffle = legacyValue('soundgrammy-shuffle')
  const mode = legacyValue('soundgrammy-shuffle-mode')
  return { shuffle: isShuffleState(shuffle) ? shuffle : 'off', mode: isShuffleMode(mode) ? mode : 'random' }
}
