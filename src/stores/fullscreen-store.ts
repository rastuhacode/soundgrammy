import { getCurrentWindow } from '@tauri-apps/api/window'
import { create } from 'zustand'
import {
  DEFAULT_BOUNCE_SETTINGS,
  loadBounceSettings,
  saveBounceSettings,
  type BounceSettings,
} from '@/lib/bounce'

export const KEEP_DISPLAY_AWAKE_KEY = 'soundgrammy-fullscreen-keep-display-awake-v1'

export function loadKeepDisplayAwake(): boolean {
  if (typeof window === 'undefined') return true
  const stored = window.localStorage.getItem(KEEP_DISPLAY_AWAKE_KEY)
  return stored === null ? true : stored !== 'false'
}

function saveKeepDisplayAwake(enabled: boolean) {
  if (typeof window === 'undefined') return
  window.localStorage.setItem(KEEP_DISPLAY_AWAKE_KEY, String(enabled))
}

interface FullscreenState {
  isFullscreen: boolean
  isTransitioning: boolean
  keepDisplayAwake: boolean
  bounce: BounceSettings
  setKeepDisplayAwake: (enabled: boolean) => void
  setBounceSettings: (patch: Partial<BounceSettings>) => void
  resetBounceSettings: () => void
  enterFullscreen: () => Promise<boolean>
  exitFullscreen: () => Promise<boolean>
  toggleFullscreen: () => Promise<boolean>
  syncFullscreen: () => Promise<void>
}

export const useFullscreenStore = create<FullscreenState>((set, get) => ({
  isFullscreen: false,
  isTransitioning: false,
  keepDisplayAwake: loadKeepDisplayAwake(),
  bounce: loadBounceSettings(),

  setKeepDisplayAwake: (enabled) => {
    saveKeepDisplayAwake(enabled)
    set({ keepDisplayAwake: enabled })
  },

  setBounceSettings: (patch) => {
    set((state) => {
      const bounce = {
        ...state.bounce,
        ...patch,
      }
      saveBounceSettings(bounce)
      return { bounce }
    })
  },

  resetBounceSettings: () => {
    const bounce = { ...DEFAULT_BOUNCE_SETTINGS }
    saveBounceSettings(bounce)
    set({ bounce })
  },

  enterFullscreen: async () => {
    if (get().isFullscreen || get().isTransitioning) return get().isFullscreen
    set({ isTransitioning: true })
    try {
      await getCurrentWindow().setFullscreen(true)
      set({ isFullscreen: true, isTransitioning: false })
      return true
    }
    catch {
      set({ isTransitioning: false })
      return false
    }
  },

  exitFullscreen: async () => {
    if (!get().isFullscreen || get().isTransitioning) return !get().isFullscreen
    set({ isTransitioning: true })
    try {
      await getCurrentWindow().setFullscreen(false)
      set({ isFullscreen: false, isTransitioning: false })
      return true
    }
    catch {
      set({ isTransitioning: false })
      return false
    }
  },

  toggleFullscreen: async () => {
    return get().isFullscreen
      ? get().exitFullscreen()
      : get().enterFullscreen()
  },

  syncFullscreen: async () => {
    try {
      const isFullscreen = await getCurrentWindow().isFullscreen()
      set({ isFullscreen, isTransitioning: false })
    }
    catch {
      // Browser previews do not expose a native Tauri window.
    }
  },
}))
