import { beforeEach, describe, expect, it, vi } from 'vitest'
import { BOUNCE_SETTINGS_KEY, DEFAULT_BOUNCE_SETTINGS, loadBounceSettings } from '@/lib/bounce'
import {
  KEEP_DISPLAY_AWAKE_KEY,
  loadKeepDisplayAwake,
  useFullscreenStore,
} from './fullscreen-store'

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    setFullscreen: vi.fn(async () => {}),
    isFullscreen: vi.fn(async () => false),
  })),
}))

const values = new Map<string, string>()

beforeEach(() => {
  values.clear()
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: {
      localStorage: {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => values.set(key, value),
      },
    },
  })
  useFullscreenStore.setState({
    bounce: { ...DEFAULT_BOUNCE_SETTINGS },
    keepDisplayAwake: true,
  })
})

describe('fullscreen display wake setting', () => {
  it('defaults to enabled when no preference is stored', () => {
    expect(loadKeepDisplayAwake()).toBe(true)
  })

  it('persists an explicit opt-out', () => {
    useFullscreenStore.getState().setKeepDisplayAwake(false)

    expect(useFullscreenStore.getState().keepDisplayAwake).toBe(false)
    expect(values.get(KEEP_DISPLAY_AWAKE_KEY)).toBe('false')
    expect(loadKeepDisplayAwake()).toBe(false)
  })
})

describe('fullscreen dynamic setting', () => {
  it('persists the single enabled preference', () => {
    useFullscreenStore.getState().setBounceEnabled(true)
    expect(useFullscreenStore.getState().bounce).toEqual({ enabled: true })
    expect(values.get(BOUNCE_SETTINGS_KEY)).toBe('{"enabled":true}')
    expect(loadBounceSettings()).toEqual({ enabled: true })
    useFullscreenStore.getState().setBounceEnabled(false)
    expect(loadBounceSettings()).toEqual({ enabled: false })
  })
})
