// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { expect, it, vi } from 'vitest'
import { useAudioControls } from './use-audio-controls'

it('does not claim browser media controls on mount or after reattachment', async () => {
  const mediaSession = vi.fn(() => ({ setActionHandler: vi.fn() }))
  Object.defineProperty(navigator, 'mediaSession', { configurable: true, get: mediaSession })
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  function Controls() {
    useAudioControls({ currentTime: 10, duration: 120, handleSeek: vi.fn() })
    return null
  }
  try {
    for (let i = 0; i < 2; i++) {
      const root = createRoot(document.createElement('div'))
      await act(async () => root.render(createElement(Controls)))
      await act(async () => root.unmount())
    }
    expect(mediaSession).not.toHaveBeenCalled()
  }
  finally {
    vi.unstubAllGlobals()
    Reflect.deleteProperty(navigator, 'mediaSession')
  }
})
