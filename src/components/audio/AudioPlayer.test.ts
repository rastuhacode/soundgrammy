// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useFullscreenStore } from '@/stores/fullscreen-store'
import { AudioPlayer } from './AudioPlayer'

const setFullscreen = vi.hoisted(() => vi.fn(async () => {}))

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ setFullscreen }),
}))
vi.mock('@/hooks/audio/engine-factory', () => ({
  AudioEngineProvider: ({ children }: { children: React.ReactNode }) => children,
}))
vi.mock('@/hooks/use-audio-engine', () => ({
  useAudioEngine: () => ({
    currentTime: 0,
    duration: 0,
    bufferedRanges: [],
    showInitialLoading: false,
    isSeeking: false,
    volume: 1,
    handleSeek: vi.fn(),
    handleSeekStart: vi.fn(),
    handleSeekEnd: vi.fn(),
    handleVolumeChange: vi.fn(),
    handleMuteToggle: vi.fn(),
    isActuallyPlaying: false,
  }),
}))
vi.mock('@/hooks/use-compact-display', () => ({
  useCompactDisplay: () => ({ isCompact: false }),
}))
vi.mock('@/stores/player-store', () => ({
  usePlayerStore: (selector: (state: unknown) => unknown) => selector({
    currentTrack: { id: 1, title: 'Test track' },
    commandError: null,
  }),
}))
vi.mock('../fullscreen/AudioFullscreenPlayer', () => ({ AudioFullscreenPlayer: () => null }))
vi.mock('./AudioPlayerBar', () => ({ AudioPlayerBar: () => null }))
vi.mock('./AudioPlayerDrawer', () => ({ AudioPlayerDrawer: () => null }))

describe('AudioPlayer fullscreen behavior', () => {
  const container = document.createElement('div')
  const root = createRoot(container)

  beforeEach(() => {
    setFullscreen.mockClear()
    useFullscreenStore.setState({ isFullscreen: false, isTransitioning: false })
  })

  afterEach(async () => {
    await act(async () => root.render(null))
  })

  it('stays fullscreen on a non-compact display', async () => {
    await act(async () => root.render(createElement(AudioPlayer)))
    await act(async () => {
      await useFullscreenStore.getState().enterFullscreen()
    })

    expect(useFullscreenStore.getState().isFullscreen).toBe(true)
    expect(setFullscreen).toHaveBeenCalledTimes(1)
    expect(setFullscreen).toHaveBeenCalledWith(true)
  })
})
