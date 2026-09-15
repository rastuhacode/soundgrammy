import type {
  AudioEngine, AudioEngineError, AudioEngineEvent, AudioEngineKind,
  AudioEngineSnapshot, AudioTrackRequest,
} from './engine'

/** Private implementation port. A session is disposed synchronously on replacement. */
export interface TransportSession {
  play(): void
  pause(): void
  seek(seconds: number): void
  beginSeek(): void
  endSeek(): void
  setVolume(percent: number): void
  dispose(): void
}
export type TransportUpdate = Partial<Pick<AudioEngineSnapshot,
  'status' | 'currentTimeSeconds' | 'durationSeconds' | 'bufferedRanges' | 'initialLoading'>>
export interface TransportObserver {
  state(update: TransportUpdate): void
  ended(): void
  failed(error: AudioEngineError): void
}
export interface TransportDriver {
  open(request: AudioTrackRequest, observer: TransportObserver): TransportSession
}
export type EngineErrorLogger = (context: {
  kind: AudioEngineKind
  trackId: number | null
  attemptId: string | null
  code: AudioEngineError['code']
}) => void

const nonnegative = (value: number) => Number.isFinite(value) ? Math.max(0, value) : 0

/** Guards implementation callbacks at the attempt boundary, including A → B → A. */
export class TransportEngine implements AudioEngine {
  private snapshot: AudioEngineSnapshot
  private listeners = new Set<(event: AudioEngineEvent) => void>()
  private session: TransportSession | null = null
  private generation = 0
  private transportRevision = 0
  private destroyed = false
  private desiredPlaying = false
  private endedAttempt = false
  private request: AudioTrackRequest | null = null

  constructor(
    readonly kind: AudioEngineKind,
    private driver: TransportDriver,
    private logError: EngineErrorLogger = () => {},
  ) {
    this.snapshot = Object.freeze({
      revision: 0, kind, status: 'idle', trackId: null, attemptId: null,
      currentTimeSeconds: 0, durationSeconds: 0, bufferedRanges: [],
      volumePercent: 100, error: null, initialLoading: false,
    })
  }

  getSnapshot = () => this.snapshot

  subscribe = (listener: (event: AudioEngineEvent) => void) => {
    if (this.destroyed) return () => {}
    this.listeners.add(listener)
    // Consumers can synchronously read getSnapshot(); no synthetic transitions.
    return () => {
      this.listeners.delete(listener)
    }
  }

  private emit(event: AudioEngineEvent) {
    for (const listener of [...this.listeners]) {
      if (!this.listeners.has(listener)) continue
      try {
        listener(event)
      }
      catch { /* One consumer must not interrupt transport or other consumers. */ }
    }
  }

  private publish(update: Partial<AudioEngineSnapshot>) {
    const next = { ...this.snapshot, ...update, revision: this.snapshot.revision + 1 }
    next.currentTimeSeconds = nonnegative(next.currentTimeSeconds)
    next.durationSeconds = nonnegative(next.durationSeconds)
    next.bufferedRanges = Object.freeze(next.bufferedRanges
      .filter(range => Number.isFinite(range.start) && Number.isFinite(range.end) && range.end > range.start && range.end > 0)
      .map(range => Object.freeze({ start: Math.max(0, range.start), end: range.end }))) as unknown as AudioEngineSnapshot['bufferedRanges']
    this.snapshot = Object.freeze(next)
    this.emit({ type: 'state', snapshot: this.snapshot })
  }

  private release() {
    ++this.generation // Invalidate before disposal, which can emit pause/error.
    const old = this.session
    this.session = null
    old?.dispose()
  }

  async load(request: AudioTrackRequest) {
    if (this.destroyed) return
    if (this.request?.attemptId === request.attemptId
      && this.request.trackId === request.trackId && this.snapshot.status !== 'error') return
    this.release()
    this.request = { ...request }
    this.desiredPlaying = false
    this.endedAttempt = false
    const generation = this.generation
    const active = () => !this.destroyed && generation === this.generation
    const failed = (error: AudioEngineError) => {
      if (!active() || this.snapshot.status === 'error') return
      this.desiredPlaying = false
      // Publish before pause: synchronous media callbacks must not hide the error.
      this.publish({ status: 'error', error: Object.freeze({ ...error }), initialLoading: false })
      if (active()) this.session?.pause()
      this.logError({ kind: this.kind, trackId: request.trackId, attemptId: request.attemptId, code: error.code })
    }
    this.publish({
      trackId: request.trackId, attemptId: request.attemptId, status: 'loading',
      currentTimeSeconds: 0, durationSeconds: nonnegative(request.expectedDurationSeconds ?? 0),
      bufferedRanges: [], error: null, initialLoading: true,
    })
    // A state listener may synchronously replace/unload the request.
    if (!active()) return
    try {
      const session = this.driver.open(this.request, {
        state: (update) => {
          if (!active() || this.snapshot.status === 'error' || this.endedAttempt) return
          if (update.status === 'paused') this.desiredPlaying = false
          if (update.status === 'playing') this.desiredPlaying = true
          this.publish(update)
        },
        failed,
        ended: () => {
          if (!active() || this.endedAttempt || this.snapshot.status === 'error') return
          this.endedAttempt = true
          this.desiredPlaying = false
          this.publish({ status: 'ended', initialLoading: false,
            currentTimeSeconds: this.snapshot.durationSeconds || this.snapshot.currentTimeSeconds })
          if (!active()) return
          this.emit({ type: 'ended', trackId: request.trackId, attemptId: request.attemptId, revision: this.snapshot.revision })
        },
      })
      if (!active()) {
        session.dispose()
        return
      }
      this.session = session
      session.setVolume(this.snapshot.volumePercent)
      if (this.snapshot.status === 'error') session.pause()
      else if (this.desiredPlaying) session.play()
    }
    catch { failed({ code: 'source-unavailable', message: 'Could not prepare audio.', recoverable: true }) }
  }

  async unload() {
    if (this.destroyed) return
    this.release()
    this.request = null
    this.desiredPlaying = false
    if (this.snapshot.status === 'idle') return
    this.publish({ status: 'idle', trackId: null, attemptId: null, currentTimeSeconds: 0,
      durationSeconds: 0, bufferedRanges: [], error: null, initialLoading: false })
  }

  async play() {
    if (this.destroyed || !this.request) return
    const command = ++this.transportRevision
    if (this.snapshot.status === 'error') {
      if (!this.snapshot.error?.recoverable) return
      const pending = this.load(this.request)
      const generation = this.generation
      await pending
      if (generation !== this.generation || command !== this.transportRevision) return
    }
    if (this.destroyed || this.desiredPlaying) return
    this.desiredPlaying = true
    this.session?.play()
  }

  async pause() {
    if (this.destroyed || !this.session) return
    ++this.transportRevision
    this.desiredPlaying = false
    this.session.pause()
  }

  async seek(seconds: number) {
    if (this.destroyed) return
    if (!Number.isFinite(seconds)) throw new RangeError('Seek time must be finite')
    if (!this.session) return
    const duration = this.snapshot.durationSeconds
    const target = Math.min(Math.max(0, seconds), duration > 0 ? duration : Infinity)
    // An ended attempt cannot emit another completion, but explicit seek still updates its timeline.
    if (this.endedAttempt) this.publish({ currentTimeSeconds: target })
    this.session.seek(target)
  }

  async beginSeek() {
    if (!this.destroyed) this.session?.beginSeek()
  }

  async endSeek() {
    if (!this.destroyed) this.session?.endSeek()
  }

  async setVolume(percent: number) {
    if (this.destroyed) return
    if (!Number.isFinite(percent)) throw new RangeError('Volume must be finite')
    const volumePercent = Math.min(100, Math.max(0, percent))
    if (this.snapshot.volumePercent === volumePercent) return
    this.publish({ volumePercent })
    this.session?.setVolume(volumePercent)
  }

  async destroy() {
    if (this.destroyed) return
    this.destroyed = true
    this.listeners.clear()
    this.release()
    this.request = null
    this.desiredPlaying = false
    this.snapshot = Object.freeze({ ...this.snapshot, revision: this.snapshot.revision + 1,
      status: 'idle', trackId: null, attemptId: null, currentTimeSeconds: 0,
      durationSeconds: 0, bufferedRanges: [], error: null, initialLoading: false })
  }
}
