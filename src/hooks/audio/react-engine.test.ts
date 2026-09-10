// @vitest-environment jsdom
import { act, createElement, StrictMode, useEffect } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { usePlayerStore } from '@/stores/player-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { useLastFmStore } from '@/stores/lastfm-store'
import { useRepeatStore } from '@/stores/repeat-store'
import type { Track } from '@/types'
import { AudioEngineProvider } from './engine-factory'
import { useAudioEngine } from './use-audio-engine'
import { createFakeAudioEngine } from './fake-engine'
import { HtmlEngineHost } from './html/HtmlEngineHost'
import { HtmlDriver } from './html/html-driver'
import { TransportEngine } from './transport-engine'

const mocks = vi.hoisted(() => ({ dispose: vi.fn(), unlisten: vi.fn() }))
vi.mock('@/hooks/use-cached-thumbnail', () => ({ useCachedThumbnail: () => ({ url: null }) }))
vi.mock('@/lib/api', () => ({
  api: {
    getTrackSource: vi.fn(async (trackId: number) => ({ kind: 'stream', trackId, total: 1000, mimeType: 'audio/mpeg' })),
    closeStreamSession: vi.fn(async () => {}),
    recordListenStart: vi.fn(async () => {}),
    recordListenEnd: vi.fn(async () => null),
    lastFmAttemptStarted: vi.fn(async () => {}),
    lastFmAttemptQualified: vi.fn(async () => {}),
    lastFmAttemptEnded: vi.fn(async () => {}),
  },
  fileSrc: () => 'https://asset.invalid/test.mp3',
  onDownloadProgress: vi.fn(async () => mocks.unlisten),
}))
vi.mock('./html/mse-session', () => ({
  resolveMseMimeType: () => 'audio/mpeg',
  attachMseSession: () => ({
    dispose: mocks.dispose, notifyProgress: vi.fn(),
    seekToTime: async () => {}, snapToBufferedTime: () => null, landToBufferedTime: () => null,
  }),
}))

const track = (id: number): Track => ({ id, duration: 120, title: 'Track', performer: null,
  tg_user_id: 1, file_id: '', file_unique_id: '', source: 'saved_music',
  mime_type: 'audio/mpeg', file_size: 100, created_at: '' })
let container: HTMLDivElement
let root: Root
let model: ReturnType<typeof useAudioEngine>
function Probe() {
  const current = useAudioEngine()
  useEffect(() => {
    model = current
  })
  return createElement('output', null, `${current.status}:${current.currentTime}`)
}
const settle = () => act(async () => {
  await Promise.resolve()
})

beforeEach(() => {
  vi.clearAllMocks()
  localStorage.clear()
  usePlayerStore.setState({ currentTrack: null, isPlaying: false, listenAttemptEpoch: 0,
    queue: { source: null, tracks: [], cursor: -1, sourceIndices: null, baseEntries: null } })
  useRepeatStore.setState({ repeat: 'none' })
  useListenStatsStore.setState({ enabled: true })
  useLastFmStore.setState({ status: null })
  container = document.createElement('div')
  document.body.append(container)
  root = createRoot(container)
  vi.spyOn(HTMLMediaElement.prototype, 'load').mockImplementation(() => {})
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {})
  vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined)
})
afterEach(async () => {
  await act(async () => {
    root.unmount()
  })
  container.remove()
  vi.restoreAllMocks()
})

describe('React engine composition', () => {
  it('drives the complete facade with a fake and no audio element, including persisted mute restore', async () => {
    localStorage.setItem('soundgrammy-volume', '37')
    const fake = createFakeAudioEngine()
    const factory = () => ({ engine: fake.engine })
    await act(async () => {
      root.render(createElement(AudioEngineProvider, { factory }, createElement(Probe)))
    })
    const a = track(1)
    await act(async () => {
      usePlayerStore.setState({ currentTrack: a, isPlaying: true })
    })
    expect(container.querySelector('audio')).toBeNull()
    expect(api.getTrackSource).not.toHaveBeenCalled()
    expect(model.isActuallyPlaying).toBe(true)
    expect(model.volume).toBe(37)
    await act(async () => {
      model.handleMuteToggle()
    })
    expect(fake.engine.getSnapshot().volumePercent).toBe(0)
    await act(async () => {
      model.handleMuteToggle()
      model.handleSeek(42)
    })
    expect(fake.engine.getSnapshot().volumePercent).toBe(37)
    expect(localStorage.getItem('soundgrammy-volume')).toBe('37')
    expect(model.currentTime).toBe(42)
  })

  it('keeps one reusable engine in Strict Mode and destroys every created lifetime', async () => {
    const instances: ReturnType<typeof createFakeAudioEngine>[] = []
    const factory = () => {
      const fake = createFakeAudioEngine()
      instances.push(fake)
      return { engine: fake.engine }
    }
    await act(async () => {
      root.render(createElement(StrictMode, null,
        createElement(AudioEngineProvider, { factory }, createElement(Probe))))
    })
    await act(async () => {
      usePlayerStore.setState({ currentTrack: track(1), isPlaying: true })
    })
    const count = instances.length
    await act(async () => {
      usePlayerStore.setState({ currentTrack: track(2) })
    })
    expect(instances).toHaveLength(count)
    expect(instances.flatMap(instance => instance.driver.sessions).filter(session => !session.disposed)).toHaveLength(1)
    await act(async () => {
      root.unmount()
    })
    expect(instances.flatMap(instance => instance.driver.sessions).every(session => session.disposed)).toBe(true)
    // afterEach can safely unmount again.
  })

  it('records rapid track attempts and closes local and Last.fm activity on unmount', async () => {
    useLastFmStore.setState({ status: { state: 'connected', username: 'test', enabled: true,
      pendingCount: 0, retainedQueues: [], lastScrobbleAtMs: null, lastError: null, lastMetadataWarning: null } })
    const fake = createFakeAudioEngine()
    const factory = () => ({ engine: fake.engine })
    await act(async () => {
      root.render(createElement(AudioEngineProvider, { factory }, createElement(Probe)))
    })
    await act(async () => {
      usePlayerStore.setState({ currentTrack: track(1), isPlaying: true })
    })
    await act(async () => {
      usePlayerStore.setState({ currentTrack: track(2) })
      usePlayerStore.setState({ currentTrack: track(1) })
    })
    expect(vi.mocked(api.recordListenStart).mock.calls.map(args => args[0])).toEqual([1, 2, 1])
    expect(api.lastFmAttemptStarted).toHaveBeenCalledTimes(2)
    await act(async () => {
      root.unmount()
    })
    expect(api.recordListenEnd).toHaveBeenCalledTimes(3)
    expect(api.lastFmAttemptEnded).toHaveBeenCalledTimes(2)
    expect(vi.mocked(api.recordListenEnd).mock.calls.at(-1)?.[0].endReason).toBe('interrupted')
  })

  it('waits for asynchronous destruction before creating a replacement engine', async () => {
    const first = createFakeAudioEngine()
    let finishDestroy!: () => void
    vi.spyOn(first.engine, 'destroy').mockImplementation(() => new Promise<void>((resolve) => {
      finishDestroy = resolve
    }))
    const factory = () => ({ engine: first.engine })
    const replacement = vi.fn(() => ({ engine: createFakeAudioEngine().engine }))
    await act(async () => {
      root.render(createElement(AudioEngineProvider, { factory }, createElement(Probe)))
    })
    await act(async () => {
      root.render(createElement(AudioEngineProvider, { factory: replacement }, createElement(Probe)))
    })
    expect(replacement).not.toHaveBeenCalled()
    await act(async () => {
      finishDestroy()
    })
    expect(replacement).toHaveBeenCalledTimes(1)
  })

  it('closes a backend source that finishes opening after replacement', async () => {
    let finishSource!: (source: Awaited<ReturnType<typeof api.getTrackSource>>) => void
    vi.mocked(api.getTrackSource).mockImplementationOnce(() => new Promise((resolve) => {
      finishSource = resolve
    }))
    const driver = new HtmlDriver()
    const engine = new TransportEngine('html', driver)
    await act(async () => {
      root.render(createElement(HtmlEngineHost, { driver }))
      await engine.load({ trackId: 1, attemptId: 'first', expectedDurationSeconds: 120 })
    })
    await settle()
    const oldId = vi.mocked(api.getTrackSource).mock.calls[0]![1]
    await act(async () => {
      await engine.load({ trackId: 2, attemptId: 'second', expectedDurationSeconds: 120 })
    })
    await settle()
    await act(async () => {
      finishSource({ kind: 'stream', trackId: 1, sessionId: oldId, total: 1000, mimeType: 'audio/mpeg' })
    })
    expect(api.closeStreamSession).toHaveBeenCalledWith(oldId)
    expect(engine.getSnapshot().trackId).toBe(2)
    expect(container.querySelectorAll('audio')).toHaveLength(1)
    await act(async () => {
      await engine.destroy()
    })
  })

  it('closes the active stream and listeners on pagehide and releases the host on destroy', async () => {
    const driver = new HtmlDriver()
    const engine = new TransportEngine('html', driver)
    await act(async () => {
      root.render(createElement(StrictMode, null, createElement(HtmlEngineHost, { driver })))
      await engine.load({ trackId: 1, attemptId: 'one', expectedDurationSeconds: 120 })
    })
    await settle()
    expect(container.querySelectorAll('audio')).toHaveLength(1)
    const sessionId = vi.mocked(api.getTrackSource).mock.calls.at(-1)![1]
    window.dispatchEvent(new Event('pagehide'))
    expect(api.closeStreamSession).toHaveBeenCalledWith(sessionId)
    expect(mocks.dispose).toHaveBeenCalledTimes(1)
    expect(mocks.unlisten).toHaveBeenCalled()
    await act(async () => {
      await engine.destroy()
    })
    expect(container.querySelector('audio')).toBeNull()
    expect(mocks.dispose).toHaveBeenCalledTimes(1)
  })
})
