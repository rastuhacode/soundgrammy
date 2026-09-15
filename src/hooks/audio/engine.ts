/** Transport only. All values crossing this boundary are serializable. */
export type AudioEngineKind = 'native-rust' | 'test'
export type AudioEngineStatus = 'idle' | 'loading' | 'ready' | 'playing' | 'buffering' | 'paused' | 'ended' | 'error'
export interface AudioTrackRequest {
  trackId: number
  attemptId: string
  expectedDurationSeconds?: number | null
}
export interface AudioBufferedRange { start: number, end: number }
export interface AudioEngineError {
  code: 'source-unavailable' | 'unsupported-format' | 'decode-failed' | 'output-unavailable' | 'interrupted' | 'unknown'
  message: string
  recoverable: boolean
}
export interface AudioEngineSnapshot {
  revision: number
  kind: AudioEngineKind
  status: AudioEngineStatus
  trackId: number | null
  attemptId: string | null
  currentTimeSeconds: number
  durationSeconds: number
  bufferedRanges: AudioBufferedRange[]
  volumePercent: number
  error: AudioEngineError | null
  /** Initial source acquisition, distinct from later buffering. */
  initialLoading: boolean
  /** Requested cursor is provisional until the transport lands. */
  seeking?: boolean
}
export type AudioEngineEvent
  = { type: 'state', snapshot: AudioEngineSnapshot }
    | { type: 'ended', trackId: number, attemptId: string, revision: number }
export interface AudioEngine {
  readonly kind: AudioEngineKind
  load(request: AudioTrackRequest): Promise<void>
  unload(): Promise<void>
  play(): Promise<void>
  pause(): Promise<void>
  seek(seconds: number): Promise<void>
  /** Scrub boundaries; implementations without seek batching may no-op. */
  beginSeek(): Promise<void>
  endSeek(): Promise<void>
  setVolume(percent: number): Promise<void>
  getSnapshot(): AudioEngineSnapshot
  subscribe(listener: (event: AudioEngineEvent) => void): () => void
  destroy(): Promise<void>
}
