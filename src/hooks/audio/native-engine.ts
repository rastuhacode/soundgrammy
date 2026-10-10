import { nativeAudioTransport } from '@/lib/native-playback'
import type { AudioTransportPort } from '@/types/audio'
import { nativeAudioSnapshotSchema, audioEndedEventSchema } from '@/types/audio'
import { appLogger } from '@/lib/app-logger'
import { contractIssues } from '@/lib/errors'
import { useCacheStore } from '@/stores/cache-store'
import type { AudioEngine, AudioEngineEvent, AudioEngineSnapshot, AudioTrackRequest } from './engine'

export type NativeTransport = AudioTransportPort

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
  private resumeCleanup: (() => void) | null = null
  private scrubbing = false
  private pendingSeek: number | null = null
  private previewTarget: number | null = null
  constructor(private transport: NativeTransport = nativeAudioTransport) {
    // Subscribe first; the fetched revision can never overwrite a newer event.
    this.queue = this.initialize()
    void this.queue.catch(() => {})
  }

  ready() { return this.queue }

  private async initialize() {
    try {
      const stateListener = await this.transport.state(value => this.accept(value))
      if (this.destroyed) {
        stateListener()
        return
      }
      this.unlisten.push(stateListener)
      const eventListener = await this.transport.event((value) => {
        if (this.destroyed) return
        const parsed = audioEndedEventSchema.safeParse(value)
        if (!parsed.success) {
          appLogger.error({ source: 'audio', title: 'Invalid native audio event', context: { issues: contractIssues(parsed.error.issues) } })
          return
        }
        if (this.ended || this.snapshot.status === 'error') return
        const event = parsed.data
        if (event.trackId !== this.identity?.trackId || event.attemptId !== this.identity?.attemptId || (event.revision < this.remoteRevision && this.snapshot.status !== 'ended')) return
        this.ended = true
        this.emit({ ...event, revision: this.snapshot.revision })
      })
      if (this.destroyed) {
        eventListener()
        return
      }
      this.unlisten.push(eventListener)
      this.accept(await this.transport.snapshot())
      if (!this.destroyed && typeof window !== 'undefined') {
        const resume = () => {
          if (document.visibilityState === 'hidden') return
          void this.transport.snapshot().then(value => this.accept(value)).catch(() => {})
        }
        window.addEventListener('pageshow', resume)
        document.addEventListener('visibilitychange', resume)
        this.resumeCleanup = () => {
          window.removeEventListener('pageshow', resume)
          document.removeEventListener('visibilitychange', resume)
        }
      }
    }
    catch (error) {
      this.unlisten.splice(0).forEach(fn => fn())
      if (!this.destroyed) {
        this.snapshot = Object.freeze({ ...this.snapshot, revision: this.snapshot.revision + 1,
          status: 'error', error: { code: 'interrupted' as const, message: 'Could not attach to native playback. Restart SoundGrammy to reconnect.', recoverable: true },
        })
        this.emit({ type: 'state', snapshot: this.snapshot })
      }
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
    if (this.destroyed) return
    const parsed = nativeAudioSnapshotSchema.safeParse(value)
    if (!parsed.success) {
      appLogger.error({ source: 'audio', title: 'Invalid native audio snapshot', context: { issues: contractIssues(parsed.error.issues) } })
      return
    }
    const next = parsed.data
    const previous = this.snapshot
    // Queue and transport have independent revisions: a position tick may overtake
    // a full resume snapshot without containing its queue. Still hydrate that queue.
    const player = next.player && next.player.revision > (this.snapshot.player?.revision ?? -1)
      ? next.player
      : this.snapshot.player
    if (next.revision <= this.remoteRevision) {
      if (player !== this.snapshot.player) {
        this.snapshot = Object.freeze({ ...this.snapshot, player, revision: this.snapshot.revision + 1 })
        this.emit({ type: 'state', snapshot: this.snapshot })
      }
      return
    }
    if (next.attemptId !== this.identity?.attemptId) {
      this.ended = false
      this.previewTarget = null
      this.pendingSeek = null
      this.scrubbing = false
      ++this.seekRevision
    }
    this.identity = next.trackId === null || next.attemptId === null ? null : { trackId: next.trackId, attemptId: next.attemptId }
    this.remoteRevision = next.revision
    if (next.status === 'error') this.previewTarget = null
    const preview = this.previewTarget === null ? {} : { currentTimeSeconds: this.previewTarget, seeking: true, status: this.scrubbing ? next.status : 'buffering' as const }
    this.snapshot = Object.freeze({ ...next, player, ...preview, revision: this.snapshot.revision + 1 })
    if (this.snapshot.status === 'error' && (previous.status !== 'error' || previous.attemptId !== this.snapshot.attemptId)) {
      appLogger.error({
        source: 'audio', title: 'Native audio playback failed',
        description: this.snapshot.error?.message,
        context: {
          trackId: this.snapshot.trackId,
          attemptId: this.snapshot.attemptId,
          previousAttemptId: previous.attemptId,
          desiredPlaying: this.snapshot.player?.isPlaying,
          queueCursor: this.snapshot.player?.queue.cursor,
          nativeRevision: next.revision,
          errorCode: this.snapshot.error?.code,
          errorDiagnostic: this.snapshot.error?.diagnostic,
          diagnosticVersion: 2,
          fullyCached: next.trackId !== null && useCacheStore.getState().cachedIds.has(next.trackId),
          lastControl: next.lastControl,
          positionSeconds: next.currentTimeSeconds,
          durationSeconds: next.durationSeconds,
          title: player?.queue.tracks[player.queue.cursor]?.title,
          performer: player?.queue.tracks[player.queue.cursor]?.performer,
        },
      })
    }
    this.emit({ type: 'state', snapshot: this.snapshot })
  }

  private command(operation: () => Promise<unknown>, attemptId?: string) {
    if (this.destroyed) return Promise.resolve()
    const epoch = this.epoch
    const run = this.queue.then(async () => {
      if (this.destroyed || epoch !== this.epoch || (attemptId !== undefined && attemptId !== this.identity?.attemptId)) return
      const response = await operation()
      if (response !== null) this.accept(response)
    })
    this.queue = run.catch((error: unknown) => {
      if (this.destroyed || epoch !== this.epoch || (attemptId !== undefined && attemptId !== this.identity?.attemptId)) return
      const previous = this.snapshot
      this.snapshot = Object.freeze({ ...this.snapshot, revision: this.snapshot.revision + 1,
        status: 'error', initialLoading: false, seeking: false,
        error: { code: 'interrupted' as const, message: 'Native audio control failed. Try playing again.', recoverable: true },
      })
      if (previous.status !== 'error') {
        appLogger.error({
          source: 'audio', title: 'Native audio control failed',
          description: this.snapshot.error?.message,
          error,
          context: {
            trackId: this.snapshot.trackId,
            attemptId: this.snapshot.attemptId,
            desiredPlaying: this.snapshot.player?.isPlaying,
            queueCursor: this.snapshot.player?.queue.cursor,
          },
        })
      }
      this.emit({ type: 'state', snapshot: this.snapshot })
    })
    return run.catch((error: unknown) => {
      if (attemptId !== undefined && attemptId !== this.identity?.attemptId) return
      throw error
    })
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
    const attemptId = this.identity.attemptId
    const revision = ++this.seekRevision
    if (this.scrubbing) {
      this.pendingSeek = target
      return
    }
    await this.command(async () => {
      if (revision !== this.seekRevision) return null
      try {
        return await this.transport.seek(target, attemptId)
      }
      finally { if (revision === this.seekRevision) this.previewTarget = null }
    }, attemptId)
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
    this.resumeCleanup?.()
    // A view only owns subscriptions. Detaching must never stop native playback.
    // Pending registrations release themselves when they resolve. A slow command
    // must not hold subscriptions or prevent a replacement view from attaching.
    this.unlisten.splice(0).forEach(fn => fn())
  }
}
