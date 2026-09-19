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
vi.mock('@/hooks/use-cached-thumbnail', () => ({ useCachedThumbnail: () => ({ url: null }) }))
vi.mock('@/lib/api', () => ({
  api: {
    listListenStats: vi.fn(async () => []),
    recordListenStart: vi.fn(async () => {}),
    recordListenEnd: vi.fn(async () => null),
    lastFmAttemptStarted: vi.fn(async () => {}),
    lastFmAttemptQualified: vi.fn(async () => {}),
    lastFmAttemptEnded: vi.fn(async () => {}),
  },
  fileSrc: () => 'https://asset.invalid/test.mp3',
  onNativeListenStats: vi.fn(async () => () => {}),
  onNativeAudioState: vi.fn(async () => () => {}),
  onNativeAudioEvent: vi.fn(async () => () => {}),
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
      await fake.engine.load({ trackId: 1, attemptId: 'native:1' })
      await fake.engine.play()
    })
    expect(container.querySelector('audio')).toBeNull()
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
      const engine = instances.at(-1)!.engine
      await engine.load({ trackId: 1, attemptId: 'native:1' })
      await engine.play()
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

  it('never starts or closes native listen attempts on UI transitions or unmount', async () => {
    useLastFmStore.setState({ status: { state: 'connected', username: 'test', enabled: true,
      pendingCount: 0, retainedQueues: [], lastScrobbleAtMs: null, lastError: null, lastMetadataWarning: null } })
    const fake = createFakeAudioEngine()
    const factory = () => ({ engine: fake.engine })
    await act(async () => {
      root.render(createElement(AudioEngineProvider, { factory }, createElement(Probe)))
    })
    await act(async () => {
      usePlayerStore.setState({ currentTrack: track(1), isPlaying: true })
      const engine = fake.engine
      await engine.load({ trackId: 1, attemptId: 'native:1' })
      await engine.play()
    })
    await act(async () => {
      usePlayerStore.setState({ currentTrack: track(2) })
      await fake.engine.load({ trackId: 2, attemptId: 'native:2' })
      usePlayerStore.setState({ currentTrack: track(1) })
      await fake.engine.load({ trackId: 1, attemptId: 'native:3' })
      await fake.engine.play()
    })
    expect(api.recordListenStart).not.toHaveBeenCalled()
    expect(api.lastFmAttemptStarted).not.toHaveBeenCalled()
    await act(async () => {
      root.unmount()
    })
    expect(api.recordListenEnd).not.toHaveBeenCalled()
    expect(api.lastFmAttemptEnded).not.toHaveBeenCalled()
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
})
