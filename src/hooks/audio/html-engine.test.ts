// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { describe, expect, it, vi } from 'vitest'
import type { AudioEngineStatus } from './engine'
import { audioEngineContract } from './engine.contract'
import { HtmlEngineHost } from './html/HtmlEngineHost'
import { HtmlDriver } from './html/html-driver'
import { TransportEngine } from './transport-engine'

vi.mock('@/lib/api', () => ({
  api: {
    getTrackSource: vi.fn(async () => ({ kind: 'cached', path: '/cache/test.mp3' })),
    closeStreamSession: vi.fn(async () => {}),
  },
  fileSrc: () => 'https://asset.invalid/test.mp3',
  onDownloadProgress: vi.fn(async () => () => {}),
}))

// Media decoding is supplied by the browser, not jsdom. The real source/seek/host
// lifecycle runs here; only the media element's platform behavior is simulated.
async function createHtmlHarness() {
  const container = document.createElement('div')
  document.body.append(container)
  const root = createRoot(container)
  const driver = new HtmlDriver()
  const engine = new TransportEngine('html', driver)
  const elements: HTMLAudioElement[] = []
  const ready = new WeakSet<HTMLAudioElement>()
  vi.spyOn(HTMLMediaElement.prototype, 'load').mockImplementation(() => {})
  vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(function (this: HTMLMediaElement) {
    Object.defineProperty(this, 'paused', { configurable: true, value: false })
    this.dispatchEvent(new Event('playing'))
    return Promise.resolve()
  })
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(function (this: HTMLMediaElement) {
    Object.defineProperty(this, 'paused', { configurable: true, value: true })
    this.dispatchEvent(new Event('pause'))
  })
  await act(async () => {
    root.render(createElement(HtmlEngineHost, { driver }))
  })
  const settle = async () => {
    await act(async () => {
      await Promise.resolve()
    })
    const audio = container.querySelector('audio')
    if (!audio || !audio.src || ready.has(audio)) return
    ready.add(audio)
    elements.push(audio)
    Object.defineProperties(audio, {
      duration: { configurable: true, value: 120 },
      readyState: { configurable: true, value: 4 },
      buffered: { configurable: true, value: { length: 1, start: () => 0, end: () => 120 } },
    })
    await act(async () => {
      audio.dispatchEvent(new Event('loadedmetadata'))
      audio.dispatchEvent(new Event('canplay'))
    })
  }
  return {
    engine, settle, elements, container,
    status: (index: number, status: AudioEngineStatus) => {
      elements[index]!.dispatchEvent(new Event(status === 'paused' ? 'pause' : status))
    },
    ended: (index: number) => {
      const audio = elements[index]!
      Object.defineProperty(audio, 'ended', { configurable: true, value: true })
      audio.dispatchEvent(new Event('ended'))
    },
    fail: (index: number) => elements[index]!.dispatchEvent(new Event('error')),
    cleanup: async () => {
      await engine.destroy()
      await act(async () => {
        root.unmount()
      })
      container.remove()
      vi.restoreAllMocks()
    },
  }
}

audioEngineContract('HTML host', createHtmlHarness)

describe('HTML transport races', () => {
  it('does not resume when an earlier play promise resolves after Pause', async () => {
    const h = await createHtmlHarness()
    try {
      await h.engine.load({ trackId: 1, attemptId: 'first', expectedDurationSeconds: 120 })
      await h.settle()
      let resolvePlay!: () => void
      vi.mocked(HTMLMediaElement.prototype.play).mockImplementationOnce(() => new Promise<void>((resolve) => {
        resolvePlay = resolve
      }))
      await h.engine.play()
      await h.engine.pause()
      const audio = h.elements[0]!
      Object.defineProperty(audio, 'paused', { configurable: true, value: false })
      audio.dispatchEvent(new Event('playing'))
      resolvePlay()
      await h.settle()
      expect(audio.paused).toBe(true)
      expect(h.engine.getSnapshot().status).toBe('paused')
    }
    finally {
      await h.cleanup()
    }
  })

  it('does not emit natural ended when seeking to the duration', async () => {
    const h = await createHtmlHarness()
    try {
      await h.engine.load({ trackId: 1, attemptId: 'first', expectedDurationSeconds: 120 })
      await h.settle()
      let ended = 0
      h.engine.subscribe((event) => {
        if (event.type === 'ended') ended++
      })
      await h.engine.seek(120)
      h.elements[0]!.dispatchEvent(new Event('seeked'))
      h.ended(0)
      await h.settle()
      expect(ended).toBe(0)
    }
    finally {
      await h.cleanup()
    }
  })
})
