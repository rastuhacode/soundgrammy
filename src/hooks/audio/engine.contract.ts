import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { AudioEngine, AudioEngineEvent, AudioEngineStatus } from './engine'

export interface EngineHarness {
  engine: AudioEngine
  settle(): Promise<void>
  status(index: number, status: AudioEngineStatus): void
  ended(index: number): void
  fail(index: number): void
  cleanup(): Promise<void>
}
const request = (trackId = 1, attemptId = 'first') => ({ trackId, attemptId, expectedDurationSeconds: 120 })

/** Run unchanged for each implementation; only readiness/media simulation differs. */
export function audioEngineContract(name: string, create: () => Promise<EngineHarness>) {
  describe(`${name} AudioEngine contract`, () => {
    let h: EngineHarness
    beforeEach(async () => {
      h = await create()
    })
    afterEach(async () => {
      await h.cleanup()
    })
    const load = async (trackId = 1, attemptId = 'first') => {
      await h.engine.load(request(trackId, attemptId))
      await h.settle()
    }

    it('starts idle with a stable, serializable snapshot and accepts load before readiness', async () => {
      const initial = h.engine.getSnapshot()
      expect(initial.status).toBe('idle')
      expect(initial.trackId).toBeNull()
      expect(initial.attemptId).toBeNull()
      expect(initial.currentTimeSeconds).toBe(0)
      expect(initial.durationSeconds).toBe(0)
      expect(initial.bufferedRanges).toEqual([])
      expect(initial.error).toBeNull()
      expect(h.engine.getSnapshot()).toBe(initial)
      const events: AudioEngineEvent[] = []
      h.engine.subscribe(event => events.push(event))
      await load()
      expect(events.some(event => event.type === 'state' && event.snapshot.status === 'loading')).toBe(true)
      expect(h.engine.getSnapshot().status).toBe('ready')
      expect(JSON.parse(JSON.stringify(h.engine.getSnapshot()))).toEqual(h.engine.getSnapshot())
      expect(h.engine.getSnapshot().revision).toBeGreaterThan(initial.revision)
    })

    it('makes play/pause idempotent without replacing the attempt', async () => {
      await load()
      await h.engine.play()
      await h.engine.play()
      await h.settle()
      expect(h.engine.getSnapshot().status).toBe('playing')
      await h.engine.pause()
      await h.engine.pause()
      await h.settle()
      expect(h.engine.getSnapshot().status).toBe('paused')
      expect(h.engine.getSnapshot().attemptId).toBe('first')
    })

    it('clamps seeks, preserves the latest scrub, and rejects non-finite input', async () => {
      await load()
      await h.engine.seek(-20)
      await h.settle()
      expect(h.engine.getSnapshot().currentTimeSeconds).toBe(0)
      await h.engine.seek(1000)
      await h.settle()
      expect(h.engine.getSnapshot().currentTimeSeconds).toBe(120)
      await h.engine.beginSeek()
      await h.engine.seek(10)
      await h.engine.seek(40)
      await h.engine.seek(70)
      await h.engine.endSeek()
      await h.settle()
      expect(h.engine.getSnapshot().currentTimeSeconds).toBe(70)
      await expect(h.engine.seek(NaN)).rejects.toThrow(RangeError)
      await expect(h.engine.seek(Infinity)).rejects.toThrow(RangeError)
      expect(h.engine.getSnapshot().currentTimeSeconds).toBe(70)
    })

    it('clamps volume and preserves it across loads and unloads', async () => {
      await h.engine.setVolume(-1)
      expect(h.engine.getSnapshot().volumePercent).toBe(0)
      await h.engine.setVolume(200)
      expect(h.engine.getSnapshot().volumePercent).toBe(100)
      await h.engine.setVolume(35)
      await load()
      await h.engine.unload()
      expect(h.engine.getSnapshot().volumePercent).toBe(35)
      await expect(h.engine.setVolume(NaN)).rejects.toThrow(RangeError)
    })

    it('restarts the same track under a new attempt ID', async () => {
      await load()
      await h.engine.seek(40)
      await h.settle()
      await load(1, 'duplicate-row')
      expect(h.engine.getSnapshot().currentTimeSeconds).toBe(0)
      expect(h.engine.getSnapshot().attemptId).toBe('duplicate-row')
    })

    it('ignores replaced A/B state, errors, and endings in A → B → A', async () => {
      await load()
      await load(2, 'second')
      await load(1, 'third')
      const latest = h.engine.getSnapshot()
      h.status(0, 'playing')
      h.status(1, 'paused')
      h.fail(0)
      h.ended(0)
      h.ended(1)
      await h.settle()
      expect(h.engine.getSnapshot()).toEqual(latest)
    })

    it('emits natural ended once per active attempt', async () => {
      await load()
      await h.engine.play()
      await h.settle()
      const events: AudioEngineEvent[] = []
      h.engine.subscribe(event => events.push(event))
      h.ended(0)
      h.ended(0)
      await h.settle()
      expect(events.filter(event => event.type === 'ended')).toHaveLength(1)
      expect(h.engine.getSnapshot().status).toBe('ended')
    })

    it('reports failure without ended and can recover on Play', async () => {
      await load()
      const events: AudioEngineEvent[] = []
      h.engine.subscribe(event => events.push(event))
      h.fail(0)
      h.ended(0)
      await h.settle()
      expect(h.engine.getSnapshot().status).toBe('error')
      expect(h.engine.getSnapshot().error?.recoverable).toBe(true)
      expect(events.filter(event => event.type === 'ended')).toHaveLength(0)
      await h.engine.play()
      await h.settle()
      expect(h.engine.getSnapshot().status).toBe('playing')
    })

    it('isolates listeners and honors unsubscribe', async () => {
      let count = 0
      h.engine.subscribe(() => {
        throw new Error('consumer failure')
      })
      const unsubscribe = h.engine.subscribe(() => {
        count++
      })
      await load()
      expect(count).toBeGreaterThan(0)
      unsubscribe()
      const previous = count
      await h.engine.play()
      await h.settle()
      expect(count).toBe(previous)
    })

    it('unloads repeatedly, can load again, and destroys terminally', async () => {
      await load()
      await h.engine.unload()
      await h.engine.unload()
      expect(h.engine.getSnapshot().status).toBe('idle')
      await load(2, 'second')
      await h.engine.destroy()
      await h.engine.destroy()
      const final = h.engine.getSnapshot()
      await h.engine.load(request())
      await h.engine.play()
      h.ended(1)
      await h.settle()
      expect(h.engine.getSnapshot()).toBe(final)
    })
  })
}
