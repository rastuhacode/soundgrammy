import { readLocalStorageValue } from '@mantine/hooks'
import { create } from 'zustand'
import { z } from 'zod'

const REPEAT_STORAGE_KEY = 'soundgrammy-repeat'
const repeatCycle = ['none', 'one', 'all'] as const

export const repeatSchema = z.enum(repeatCycle)
export type RepeatState = z.infer<typeof repeatSchema>

function parseStoredRepeat(stored: string | undefined): RepeatState {
  if (stored === undefined) return 'none'
  try {
    const result = repeatSchema.safeParse(JSON.parse(stored))
    if (result.success) return result.data
  }
  catch {
    const result = repeatSchema.safeParse(stored)
    if (result.success) return result.data
  }
  return 'none'
}

function persistRepeat(repeat: RepeatState) {
  if (typeof window === 'undefined') return
  localStorage.setItem(REPEAT_STORAGE_KEY, JSON.stringify(repeat))
}

interface RepeatStoreState {
  repeat: RepeatState
  setRepeat: (repeat: RepeatState) => void
  toggleRepeat: () => void
  hydrate: () => void
}

export const useRepeatStore = create<RepeatStoreState>((set, get) => ({
  repeat: 'none',

  setRepeat: (repeat) => {
    persistRepeat(repeat)
    set({ repeat })
  },

  toggleRepeat: () => {
    const { repeat } = get()
    const nextIndex = (repeatCycle.indexOf(repeat) + 1) % repeatCycle.length
    const nextRepeat = repeatCycle[nextIndex]!
    persistRepeat(nextRepeat)
    set({ repeat: nextRepeat })
  },

  hydrate: () => {
    const repeat = readLocalStorageValue<RepeatState>({
      key: REPEAT_STORAGE_KEY,
      defaultValue: 'none',
      deserialize: parseStoredRepeat,
    })
    set({ repeat })
  },
}))
