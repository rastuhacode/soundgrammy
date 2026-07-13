import { getCurrentWindow } from '@tauri-apps/api/window'
import { create } from 'zustand'

interface FullscreenState {
  isFullscreen: boolean
  isTransitioning: boolean
  enterFullscreen: () => Promise<boolean>
  exitFullscreen: () => Promise<boolean>
  toggleFullscreen: () => Promise<boolean>
  syncFullscreen: () => Promise<void>
}

export const useFullscreenStore = create<FullscreenState>((set, get) => ({
  isFullscreen: false,
  isTransitioning: false,

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
