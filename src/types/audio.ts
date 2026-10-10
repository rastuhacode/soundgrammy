import { z } from 'zod'
import { playbackSessionSchema } from '@/types/playback'
/** Native playback observation and transport controls; all payloads are serializable. */
export const audioEngineKindSchema = z.enum(['native-rust', 'test'])
export type AudioEngineKind = z.infer<typeof audioEngineKindSchema>
export const audioEngineStatusSchema = z.enum(['idle', 'loading', 'ready', 'playing', 'buffering', 'paused', 'ended', 'error'])
export type AudioEngineStatus = z.infer<typeof audioEngineStatusSchema>
export interface AudioTrackRequest {
  trackId: number
  attemptId: string
  expectedDurationSeconds?: number | null
}
export interface AudioTransportPort {
  state(listener: (value: unknown) => void): Promise<() => void>
  event(listener: (value: unknown) => void): Promise<() => void>
  snapshot(): Promise<unknown>
  load(request: AudioTrackRequest): Promise<unknown>
  unload(): Promise<unknown>
  play(): Promise<unknown>
  pause(): Promise<unknown>
  seek(seconds: number, attemptId?: string): Promise<unknown>
  volume(percent: number): Promise<unknown>
}
const nonnegative = z.number().nonnegative()
export const audioBufferedRangeSchema = z.object({ start: nonnegative, end: nonnegative })
export type AudioBufferedRange = z.infer<typeof audioBufferedRangeSchema>
export const audioEngineErrorSchema = z.object({
  code: z.enum(['source-unavailable', 'unsupported-format', 'decode-failed', 'output-unavailable', 'interrupted', 'unknown']),
  message: z.string(), recoverable: z.boolean(),
  /** Sanitized backend detail retained for opt-in diagnostic logs. */
  diagnostic: z.string().optional(),
})
export type AudioEngineError = z.infer<typeof audioEngineErrorSchema>
export const audioEngineSnapshotSchema = z.object({
  revision: nonnegative.int(), kind: audioEngineKindSchema, status: audioEngineStatusSchema,
  player: playbackSessionSchema.optional(), lastControl: z.string().nullable().optional(),
  trackId: z.number().int().nullable(), attemptId: z.string().nullable(),
  currentTimeSeconds: nonnegative, durationSeconds: nonnegative,
  bufferedRanges: z.array(audioBufferedRangeSchema), volumePercent: nonnegative.max(100),
  error: audioEngineErrorSchema.nullable(), initialLoading: z.boolean(), seeking: z.boolean().optional(),
})
export type AudioEngineSnapshot = z.infer<typeof audioEngineSnapshotSchema>
export const nativeAudioSnapshotSchema = audioEngineSnapshotSchema.extend({ kind: z.literal('native-rust') })
export const audioEndedEventSchema = z.object({
  type: z.literal('ended'), revision: nonnegative.int(), trackId: z.number().int(), attemptId: z.string(),
})

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
