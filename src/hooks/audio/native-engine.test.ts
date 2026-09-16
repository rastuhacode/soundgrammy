import { describe, expect, it, vi } from 'vitest'
import { audioEngineContract } from './engine.contract'
import { FakeAudioDriver } from './fake-engine'
import { NativeRustAudioEngine, type NativeTransport } from './native-engine'
import { TransportEngine } from './transport-engine'
import type { AudioEngineSnapshot, AudioEngineStatus } from './engine'

vi.mock('@/lib/api', () => ({ api: {}, onNativeAudioState: vi.fn(), onNativeAudioEvent: vi.fn() }))
function harness() {
  const driver = new FakeAudioDriver()
  const backend = new TransportEngine('native-rust', driver)
  let state: (v: unknown) => void = () => {}
  let edge: (v: unknown) => void = () => {}
  backend.subscribe(event => event.type === 'state' ? state(event.snapshot) : edge(event))
  const response = async (operation: Promise<void>) => {
    await operation
    return backend.getSnapshot()
  }
  const transport: NativeTransport = {
    state: async (listener) => {
      state = listener
      return () => {
        state = () => {}
      }
    },
    event: async (listener) => {
      edge = listener
      return () => {
        edge = () => {}
      }
    },
    snapshot: async () => backend.getSnapshot(),
    load: request => response(backend.load(request)),
    unload: () => response(backend.unload()),
    play: () => response(backend.play()),
    pause: () => response(backend.pause()),
    seek: seconds => response(backend.seek(seconds)),
    volume: percent => response(backend.setVolume(percent)),
  }
  const engine = new NativeRustAudioEngine(transport)
  return {
    engine, backend, transport,
    send: (value: unknown) => state(value),
    settle: async () => { await Promise.resolve() },
    status: (i: number, status: AudioEngineStatus) => driver.sessions[i]!.observer.state({ status }),
    ended: (i: number) => driver.sessions[i]!.observer.ended(),
    fail: (i: number) => driver.sessions[i]!.observer.failed({ code: 'decode-failed', message: 'Test failure', recoverable: true }),
    cleanup: () => engine.destroy(),
  }
}
audioEngineContract('native proxy with mocked Tauri', async () => harness())

describe('native event ordering', () => {
  it('rejects malformed payloads and old revisions across native identities', async () => {
    const h = harness()
    await h.engine.load({ trackId: 1, attemptId: 'new' })
    const snapshot = h.engine.getSnapshot()
    h.send({ ...snapshot, revision: 0, attemptId: 'old', status: 'playing' })
    h.send({ ...snapshot, revision: 1001, currentTimeSeconds: NaN })
    h.send({ ...snapshot, revision: 0, status: 'playing' })
    expect(h.engine.getSnapshot()).toBe(snapshot)
    await h.cleanup()
  })
  it('does not let a delayed startup snapshot overwrite a subscribed update', async () => {
    const h = harness()
    await h.engine.load({ trackId: 1, attemptId: 'a' })
    const latest = { ...h.engine.getSnapshot(), revision: 100, currentTimeSeconds: 15 } satisfies AudioEngineSnapshot
    h.send(latest)
    h.send({ ...latest, revision: 99, currentTimeSeconds: 0 })
    expect(h.engine.getSnapshot().currentTimeSeconds).toBe(15)
    await h.cleanup()
  })
  it('surfaces a failed command but detaches without unloading native playback', async () => {
    const h = harness()
    await h.engine.load({ trackId: 1, attemptId: 'a' })
    h.transport.play = vi.fn().mockRejectedValue(new Error('IPC unavailable'))
    await expect(h.engine.play()).rejects.toThrow()
    expect(h.engine.getSnapshot().error?.code).toBe('interrupted')
    h.transport.unload = vi.fn().mockRejectedValue(new Error('cannot confirm cancellation'))
    await expect(h.engine.destroy()).resolves.toBeUndefined()
    expect(h.transport.unload).not.toHaveBeenCalled()
  })
})

describe('native seek feedback', () => {
  it('shows the requested cursor immediately and ignores old positions until acknowledgement', async () => {
    const h = harness()
    await h.engine.load({ trackId: 1, attemptId: 'a', expectedDurationSeconds: 120 })
    let finish: (value: unknown) => void = () => {}
    h.transport.seek = () => new Promise((resolve) => {
      finish = resolve
    })
    const seeking = h.engine.seek(65)
    expect(h.engine.getSnapshot().currentTimeSeconds).toBe(65)
    expect(h.engine.getSnapshot().seeking).toBe(true)
    await Promise.resolve()
    h.send({ ...h.backend.getSnapshot(), revision: 500, currentTimeSeconds: 4, status: 'playing' })
    expect(h.engine.getSnapshot().currentTimeSeconds).toBe(65)
    expect(h.engine.getSnapshot().status).toBe('buffering')
    finish({ ...h.backend.getSnapshot(), revision: 501, currentTimeSeconds: 65, seeking: true, status: 'buffering' })
    await seeking
    h.send({ ...h.backend.getSnapshot(), revision: 502, currentTimeSeconds: 65.1, seeking: false, status: 'playing' })
    expect(h.engine.getSnapshot().currentTimeSeconds).toBe(65.1)
    expect(h.engine.getSnapshot().seeking).toBe(false)
    await h.cleanup()
  })
})

describe('native lifetime and reattachment', () => {
  it('adopts an already playing backend and survives view destruction', async () => {
    const h = harness()
    await h.backend.load({ trackId: 1, attemptId: 'native:41' })
    await h.backend.play()
    await h.engine.ready()
    expect(h.engine.getSnapshot().attemptId).toBe('native:41')
    expect(h.engine.getSnapshot().status).toBe('playing')
    await h.engine.destroy()
    expect(h.backend.getSnapshot().status).toBe('playing')
    await h.backend.load({ trackId: 2, attemptId: 'native:42' })
    await h.backend.play()
    const reattached = new NativeRustAudioEngine(h.transport)
    await reattached.ready()
    expect(reattached.getSnapshot().trackId).toBe(2)
    expect(reattached.getSnapshot().attemptId).toBe('native:42')
    await reattached.destroy()
    await h.backend.destroy()
  })
  it('accepts native advancement to an identity never requested by this view', async () => {
    const h = harness()
    await h.engine.ready()
    await h.backend.load({ trackId: 9, attemptId: 'native:99' })
    await h.backend.play()
    expect(h.engine.getSnapshot().trackId).toBe(9)
    expect(h.engine.getSnapshot().status).toBe('playing')
    await h.cleanup()
  })
})

it('does not turn a stale seek rejection into an error on the next native track', async () => {
  const h = harness()
  await h.engine.load({ trackId: 1, attemptId: 'native:1' })
  let reject!: (reason: Error) => void
  h.transport.seek = vi.fn(() => new Promise((_resolve, fail) => {
    reject = fail
  }))
  const seeking = h.engine.seek(65)
  await Promise.resolve()
  expect(h.transport.seek).toHaveBeenCalledWith(65, 'native:1')
  await h.backend.load({ trackId: 2, attemptId: 'native:2' })
  await h.backend.play()
  reject(new Error('Playback changed before seeking'))
  await expect(seeking).resolves.toBeUndefined()
  expect(h.engine.getSnapshot().trackId).toBe(2)
  expect(h.engine.getSnapshot().error).toBeNull()
  expect(h.engine.getSnapshot().currentTimeSeconds).toBe(0)
  await h.cleanup()
})

it('hydrates a missed queue snapshot even if a newer position tick arrived first', async () => {
  const h = harness()
  await h.engine.load({ trackId: 1, attemptId: 'native:1' })
  const player = { revision: 2, attempt: 1, endReason: 'replaced', isPlaying: true,
    preferences: { repeat: 'all', shuffle: 'on', mode: 'smart' },
    queue: { tracks: [], cursor: -1, source: null, sourceIndices: null, baseEntries: null } }
  const base = h.backend.getSnapshot()
  h.send({ ...base, revision: 100, currentTimeSeconds: 20 })
  h.send({ ...base, revision: 99, currentTimeSeconds: 19, player })
  expect(h.engine.getSnapshot().currentTimeSeconds).toBe(20)
  expect(h.engine.getSnapshot().player?.revision).toBe(2)
  h.send({ ...base, revision: 101, currentTimeSeconds: 21, player: { ...player, revision: 1 } })
  expect(h.engine.getSnapshot().player?.revision).toBe(2)
  await h.cleanup()
})
