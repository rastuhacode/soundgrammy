import { z } from 'zod'
import { api, onNativeAudioEvent, onNativeAudioState } from '@/lib/api'
import type { AudioEngine, AudioEngineEvent, AudioEngineSnapshot, AudioTrackRequest } from './engine'

const number = z.number().finite().nonnegative()
const snapshotSchema = z.object({
  revision: number.int(), kind: z.literal('native-rust'),
  status: z.enum(['idle', 'loading', 'ready', 'playing', 'buffering', 'paused', 'ended', 'error']),
  trackId: z.number().int().nullable(), attemptId: z.string().nullable(),
  currentTimeSeconds: number, durationSeconds: number,
  bufferedRanges: z.array(z.object({ start: number, end: number })),
  volumePercent: number.max(100), initialLoading: z.boolean(), seeking: z.boolean().optional(),
  error: z.object({
    code: z.enum(['source-unavailable', 'unsupported-format', 'decode-failed', 'output-unavailable', 'interrupted', 'unknown']),
    message: z.string(), recoverable: z.boolean(),
  }).nullable(),
})
const endedSchema = z.object({ type: z.literal('ended'), revision: number.int(), trackId: z.number().int(), attemptId: z.string() })
export interface NativeTransport {
  state(listener: (value: unknown) => void): Promise<() => void>
  event(listener: (value: unknown) => void): Promise<() => void>
  snapshot(): Promise<unknown>
  load(request: AudioTrackRequest): Promise<unknown>
  unload(): Promise<unknown>
  play(): Promise<unknown>
  pause(): Promise<unknown>
  seek(seconds: number): Promise<unknown>
  volume(percent: number): Promise<unknown>
}
const defaultTransport: NativeTransport = {
  state: onNativeAudioState, event: onNativeAudioEvent,
  snapshot: api.nativeAudioSnapshot, load: api.nativeAudioLoad,
  unload: api.nativeAudioUnload, play: api.nativeAudioPlay,
  pause: api.nativeAudioPause, seek: api.nativeAudioSeek, volume: api.nativeAudioSetVolume,
}
export class NativeRustAudioEngine implements AudioEngine {
  readonly kind = 'native-rust'
  private snapshot: AudioEngineSnapshot = Object.freeze({
    revision: 0, kind: this.kind, status: 'idle', trackId: null, attemptId: null,
    currentTimeSeconds: 0, durationSeconds: 0, bufferedRanges: [], volumePercent: 100,
    error: null, initialLoading: false,
  })

  private listeners = new Set<(event: AudioEngineEvent) => void>()
  private remoteRevision = -1
  private identity: AudioTrackRequest | null = null
  private epoch = 0
  private ended = false
  private destroyed = false
  private queue: Promise<void>
  private unlisten: (() => void)[] = []
  private seekRevision = 0
  private scrubbing = false
  private pendingSeek: number | null = null
  private previewTarget: number | null = null
  constructor(private transport: NativeTransport = defaultTransport) {
    // Subscribe first; the fetched revision can never overwrite a newer event.
    this.queue = this.initialize()
    void this.queue.catch(() => {})
  }

  ready() { return this.queue }

  private async initialize() {
    try {
      this.unlisten.push(await this.transport.state(value => this.accept(value)))
      this.unlisten.push(await this.transport.event((value) => {
        const parsed = endedSchema.safeParse(value)
        if (!parsed.success || this.destroyed || this.ended || this.snapshot.status === 'error') return
        const event = parsed.data
        if (event.trackId !== this.identity?.trackId || event.attemptId !== this.identity?.attemptId || (event.revision < this.remoteRevision && this.snapshot.status !== 'ended')) return
        this.ended = true
        this.emit({ ...event, revision: this.snapshot.revision })
      }))
      this.accept(await this.transport.snapshot())
    }
    catch (error) {
      this.unlisten.splice(0).forEach(fn => fn())
      throw error
    }
  }

  private emit(event: AudioEngineEvent) {
    for (const listener of this.listeners) {
      try {
        listener(event)
      }
      catch { /* Keep transport independent of consumers. */ }
    }
  }

  private accept(value: unknown) {
    const parsed = snapshotSchema.safeParse(value)
    if (!parsed.success || this.destroyed) return
    const next = parsed.data
    if (next.revision <= this.remoteRevision) return
    if (next.trackId !== (this.identity?.trackId ?? null) || next.attemptId !== (this.identity?.attemptId ?? null)) return
    this.remoteRevision = next.revision
    if (next.status === 'error') this.previewTarget = null
    const preview = this.previewTarget === null ? {} : { currentTimeSeconds: this.previewTarget, seeking: true, status: this.scrubbing ? next.status : 'buffering' as const }
    this.snapshot = Object.freeze({ ...next, ...preview, revision: this.snapshot.revision + 1 })
    this.emit({ type: 'state', snapshot: this.snapshot })
  }

  private command(operation: () => Promise<unknown>) {
    if (this.destroyed) return Promise.resolve()
    const epoch = this.epoch
    const run = this.queue.then(async () => {
      if (this.destroyed || epoch !== this.epoch) return
      this.accept(await operation())
    })
    this.queue = run.catch(() => {
      if (this.destroyed || epoch !== this.epoch) return
      this.snapshot = Object.freeze({ ...this.snapshot, revision: this.snapshot.revision + 1,
        status: 'error', initialLoading: false, seeking: false,
        error: { code: 'interrupted' as const, message: 'Native audio control failed. Try playing again.', recoverable: true },
      })
      this.emit({ type: 'state', snapshot: this.snapshot })
    })
    return run
  }

  getSnapshot = () => this.snapshot
  subscribe = (listener: (event: AudioEngineEvent) => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  load(request: AudioTrackRequest) {
    if (this.destroyed) return Promise.resolve()
    ++this.epoch
    this.identity = { ...request }
    this.ended = false
    this.pendingSeek = null
    this.scrubbing = false
    this.previewTarget = null
    return this.command(() => this.transport.load(request))
  }

  unload() {
    if (this.destroyed) return Promise.resolve()
    ++this.epoch
    this.identity = null
    this.pendingSeek = null
    this.scrubbing = false
    this.previewTarget = null
    return this.command(() => this.transport.unload())
  }

  play() { return this.command(() => this.transport.play()) }
  pause() { return this.command(() => this.transport.pause()) }
  async seek(seconds: number) {
    if (!Number.isFinite(seconds)) throw new RangeError('Seek time must be finite')
    const duration = this.snapshot.durationSeconds
    const target = Math.min(Math.max(0, seconds), duration || Infinity)
    if (this.destroyed || !this.identity) return
    this.previewTarget = target
    this.snapshot = Object.freeze({ ...this.snapshot, revision: this.snapshot.revision + 1,
      currentTimeSeconds: target, seeking: true, status: this.scrubbing ? this.snapshot.status : 'buffering',
    })
    this.emit({ type: 'state', snapshot: this.snapshot })
    const revision = ++this.seekRevision
    if (this.scrubbing) {
      this.pendingSeek = target
      return
    }
    await this.command(async () => {
      if (revision !== this.seekRevision) return null
      try {
        return await this.transport.seek(target)
      }
      finally { if (revision === this.seekRevision) this.previewTarget = null }
    })
  }

  async beginSeek() { this.scrubbing = true }
  async endSeek() {
    this.scrubbing = false
    const target = this.pendingSeek
    this.pendingSeek = null
    if (target !== null) await this.seek(target)
  }

  async setVolume(percent: number) {
    if (!Number.isFinite(percent)) throw new RangeError('Volume must be finite')
    await this.command(() => this.transport.volume(Math.min(100, Math.max(0, percent))))
  }

  async destroy() {
    if (this.destroyed) return
    this.destroyed = true
    this.listeners.clear()
    // A failed unload must reject: the provider cannot safely create another owner.
    await this.queue.catch(() => {})
    try {
      await this.transport.unload()
    }
    finally { this.unlisten.splice(0).forEach(fn => fn()) }
  }
}
