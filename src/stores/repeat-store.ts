import { isRepeatState, type RepeatState } from '@/lib/repeat'
import { readLocalStorageValue } from '@mantine/hooks'
import { create } from 'zustand'

/** Read-only UI mirror; mutations go through usePlayerStore's native commands. */
export const useRepeatStore = create<{ repeat: RepeatState }>(() => ({ repeat: 'none' }))

/** Used only when the native preference record has not yet been created. */
export function readLegacyRepeat(): RepeatState {
  return readLocalStorageValue<RepeatState>({
    key: 'soundgrammy-repeat', defaultValue: 'none',
    deserialize: (stored) => {
      try {
        const value: unknown = JSON.parse(stored ?? 'null')
        return isRepeatState(value) ? value : 'none'
      }
      catch { return isRepeatState(stored) ? stored : 'none' }
    },
  })
}
