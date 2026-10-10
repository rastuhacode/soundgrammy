// @vitest-environment jsdom
import { act, createElement, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { onBackButtonPress } from '@tauri-apps/api/app'
import { useAndroidBackAction } from './use-android-back'
import { Drawer, DrawerContent, DrawerTitle } from '@/components/ui/drawer'
import { Popover, PopoverContent } from '@/components/ui/popover'
import { androidBackStack } from '@/lib/android-back'

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('@tauri-apps/api/app', () => ({ onBackButtonPress: vi.fn() }))

Object.defineProperty(navigator, 'userAgent', { configurable: true, value: 'Android' })
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

afterEach(() => {
  vi.clearAllMocks()
})

describe('Android Back UI navigation', () => {
  it('closes a nested popover, then its drawer, then returns to playlists', async () => {
    let pressBack: (() => void) | undefined
    const unregister = vi.fn(async () => {})
    vi.mocked(onBackButtonPress).mockImplementation(async (handler) => {
      pressBack = () => handler({ canGoBack: false })
      return { unregister } as unknown as Awaited<ReturnType<typeof onBackButtonPress>>
    })

    const states: number[] = []
    const closeLog: string[] = []
    function Screen() {
      const [level, setLevel] = useState(3)
      states.push(level)
      useAndroidBackAction(level > 0, () => {
        closeLog.push('playlist')
        setLevel(0)
      })
      return createElement(Drawer, {
        open: level >= 2,
        onOpenChange: (open) => {
          if (!open) {
            closeLog.push('drawer')
            setLevel(1)
          }
        },
      }, createElement(DrawerContent, null,
        createElement(DrawerTitle, null, 'Player'),
        createElement(Popover, {
          open: level >= 3,
          onOpenChange: (open) => {
            if (!open) {
              closeLog.push('popover')
              setLevel(2)
            }
          },
        }, createElement(PopoverContent, null, 'Queue')),
      ))
    }

    const container = document.createElement('div')
    document.body.append(container)
    const root = createRoot(container)
    try {
      await act(async () => {
        root.render(createElement(Screen))
      })
      await androidBackStack.settled()
      expect(onBackButtonPress).toHaveBeenCalledTimes(1)

      await act(async () => {
        pressBack?.()
      })
      expect(states.at(-1)).toBe(2)
      await act(async () => {
        pressBack?.()
      })
      expect(closeLog).toEqual(['popover', 'drawer'])
      expect(states.at(-1)).toBe(1)
      await act(async () => {
        pressBack?.()
      })
      expect(states.at(-1)).toBe(0)
      await androidBackStack.settled()
      expect(unregister).toHaveBeenCalledTimes(1)
    }
    finally {
      await act(async () => {
        root.unmount()
      })
      container.remove()
    }
  })
})
