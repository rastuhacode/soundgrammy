import { isRepeatState, type RepeatState } from '@/lib/repeat'
import { readLocalStorageValue } from '@mantine/hooks'
import { create } from 'zustand'

interface RepeatStoreState {
  repeat: RepeatState
  setRepeat: (repeat: RepeatState) => void
  toggle: () => void
  hydrate: () => void
}

export const useRepeatStore = create<RepeatStoreState>((set, get) => {
  const defaultRepeat: RepeatState = 'none'
  const RepeatCycle = new Map<RepeatState, RepeatState>([
    ['none', 'one'],
    ['one', 'all'],
    ['all', 'none'],
  ])

  const { read, write } = useStorageRepeat()

  return {
    repeat: defaultRepeat,
    setRepeat: (repeat) => {
      write(repeat)
      set({ repeat })
    },
    toggle: () => {
      const { repeat } = get()
      const nextRepeat = RepeatCycle.get(repeat)!
      write(nextRepeat)
      set({ repeat: nextRepeat })
    },
    hydrate: () => set({ repeat: read() }),
  }
})

/**
 * Reads and writes the repeat state to localStorage.
 * @returns The read and write functions.
 */
function useStorageRepeat() {
  const REPEAT_STORAGE_KEY = 'soundgrammy-repeat'

  function deserialize(stored: string | undefined): RepeatState {
    if (stored === undefined) return 'none'
    try {
      const state = JSON.parse(stored)
      if (isRepeatState(state)) return state
    }
    catch {
      if (isRepeatState(stored)) return stored
    }
    return 'none'
  }

  const read = (): RepeatState => {
    return readLocalStorageValue<RepeatState>({
      key: REPEAT_STORAGE_KEY,
      defaultValue: 'none',
      deserialize,
    })
  }

  const write = (repeat: RepeatState) => {
    if (typeof window === 'undefined') return
    localStorage.setItem(REPEAT_STORAGE_KEY, JSON.stringify(repeat))
  }

  return { read, write }
}
