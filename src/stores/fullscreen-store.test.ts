import { beforeEach, describe, expect, it, vi } from 'vitest'
import { DEFAULT_BOUNCE_SETTINGS } from '@/lib/bounce'
import { useFullscreenStore } from './fullscreen-store'

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
  useFullscreenStore.setState({ bounce: { ...DEFAULT_BOUNCE_SETTINGS } })
})

describe('fullscreen bounce settings', () => {
  it('updates one control without replacing the others and persists it', () => {
    useFullscreenStore.getState().setBounceSettings({ strength: 82 })
    expect(useFullscreenStore.getState().bounce).toEqual({
      ...DEFAULT_BOUNCE_SETTINGS,
      strength: 82,
    })
    expect([...values.values()][0]).toContain('"strength":82')
  })

  it('resets every control to the defaults', () => {
    useFullscreenStore.getState().setBounceSettings({
      enabled: false,
      balance: 100,
      smoothness: 0,
    })
    useFullscreenStore.getState().resetBounceSettings()
    expect(useFullscreenStore.getState().bounce).toEqual(DEFAULT_BOUNCE_SETTINGS)
  })
})
