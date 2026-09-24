// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { AudioProgressBar } from './AudioProgressBar'

const roots: Array<ReturnType<typeof createRoot>> = []

afterEach(async () => {
  await act(async () => {
    roots.forEach(root => root.unmount())
  })
  roots.length = 0
  document.body.replaceChildren()
  vi.restoreAllMocks()
})

async function renderBar(userAgent: string) {
  vi.spyOn(window.navigator, 'userAgent', 'get').mockReturnValue(userAgent)
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  roots.push(root)
  await act(async () => {
    root.render(createElement(AudioProgressBar, { currentTime: 20, duration: 100, onSeek: () => {} }))
  })
  const bar = host.querySelector<HTMLElement>('[data-slot="audio-progress"]')!
  vi.spyOn(bar, 'getBoundingClientRect').mockReturnValue(DOMRect.fromRect({
    x: 0, y: 0, width: 200, height: 32,
  }))
  return bar
}

async function dispatchPointer(target: HTMLElement, type: string, pointerType: string) {
  const event = new Event(type, { bubbles: true })
  Object.defineProperties(event, {
    clientX: { value: 100 },
    clientY: { value: 16 },
    pointerType: { value: pointerType },
  })
  await act(async () => target.dispatchEvent(event))
}

describe('AudioProgressBar hover preview', () => {
  it('shows mouse seek time on desktop, but never leaves a touch preview open', async () => {
    const bar = await renderBar('Mozilla/5.0 (Macintosh)')
    await dispatchPointer(bar, 'pointermove', 'mouse')
    expect(document.querySelector('[data-slot="tooltip-content"]')?.textContent).toContain('0:50')

    await dispatchPointer(bar, 'pointermove', 'touch')
    expect(document.querySelector('[data-slot="tooltip-content"]')).toBeNull()
  })

  it('does not open the seek tooltip on Android', async () => {
    const bar = await renderBar('Mozilla/5.0 (Linux; Android 16)')
    const slider = bar.querySelector<HTMLElement>('input[type="range"]')!
    await dispatchPointer(bar, 'pointermove', 'mouse')
    await dispatchPointer(slider, 'pointerdown', 'touch')
    await dispatchPointer(slider, 'pointerup', 'touch')
    await dispatchPointer(bar, 'pointermove', 'touch')
    expect(document.querySelector('[data-slot="tooltip-content"]')).toBeNull()
  })
})
