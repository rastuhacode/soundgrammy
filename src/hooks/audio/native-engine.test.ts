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
  it('rejects malformed payloads, old revisions, and replaced identities', async () => {
    const h = harness()
    await h.engine.load({ trackId: 1, attemptId: 'new' })
    const snapshot = h.engine.getSnapshot()
    h.send({ ...snapshot, revision: 1000, attemptId: 'old', status: 'playing' })
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
  it('surfaces a failed command and requires confirmed teardown', async () => {
    const h = harness()
    await h.engine.load({ trackId: 1, attemptId: 'a' })
    h.transport.play = vi.fn().mockRejectedValue(new Error('IPC unavailable'))
    await expect(h.engine.play()).rejects.toThrow()
    expect(h.engine.getSnapshot().error?.code).toBe('interrupted')
    h.transport.unload = vi.fn().mockRejectedValue(new Error('cannot confirm cancellation'))
    await expect(h.engine.destroy()).rejects.toThrow('cannot confirm')
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
