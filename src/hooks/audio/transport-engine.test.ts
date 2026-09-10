import { describe, expect, it, vi } from 'vitest'
import { createFakeAudioEngine, FakeAudioDriver } from './fake-engine'
import { TransportEngine } from './transport-engine'

const request = { trackId: 1, attemptId: 'first', expectedDurationSeconds: 120 }
const error = { code: 'decode-failed' as const, message: 'Decode failed', recoverable: true }

describe('transport races', () => {
  it('keeps a later pause authoritative while Play recovers an error', async () => {
    const { engine, driver } = createFakeAudioEngine()
    await engine.load(request)
    driver.sessions[0]!.observer.failed(error)
    const pendingPlay = engine.play()
    await engine.pause()
    await pendingPlay
    expect(engine.getSnapshot().status).not.toBe('playing')
    await engine.destroy()
  })

  it('does not play a replacement when an earlier recovery resolves', async () => {
    const { engine, driver } = createFakeAudioEngine()
    await engine.load(request)
    driver.sessions[0]!.observer.failed(error)
    const pendingPlay = engine.play()
    await engine.load({ ...request, trackId: 2, attemptId: 'second' })
    await pendingPlay
    expect(engine.getSnapshot().status).toBe('ready')
    expect(engine.getSnapshot().trackId).toBe(2)
    await engine.destroy()
  })

  it('accepts a repeated active load without resetting position or opening another source', async () => {
    const { engine, driver } = createFakeAudioEngine()
    await engine.load(request)
    await engine.seek(30)
    await engine.load({ ...request })
    expect(driver.sessions).toHaveLength(1)
    expect(engine.getSnapshot().currentTimeSeconds).toBe(30)
    await engine.destroy()
  })

  it('logs one safe error per failure and isolates listener exceptions', async () => {
    const log = vi.fn()
    const driver = new FakeAudioDriver()
    const engine = new TransportEngine('test', driver, log)
    engine.subscribe(() => {
      throw new Error('Broken listener')
    })
    await engine.load(request)
    driver.sessions[0]!.observer.failed(error)
    driver.sessions[0]!.observer.failed(error)
    expect(log).toHaveBeenCalledExactlyOnceWith({
      kind: 'test', trackId: 1, attemptId: 'first', code: 'decode-failed',
    })
    await engine.destroy()
  })
})
