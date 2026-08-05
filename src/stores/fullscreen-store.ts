import { getCurrentWindow } from '@tauri-apps/api/window'
import { create } from 'zustand'
import {
  DEFAULT_BOUNCE_SETTINGS,
  loadBounceSettings,
  saveBounceSettings,
  type BounceSettings,
} from '@/lib/bounce'

interface FullscreenState {
  isFullscreen: boolean
  isTransitioning: boolean
  bounce: BounceSettings
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
  bounce: loadBounceSettings(),

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
