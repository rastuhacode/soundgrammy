import { z } from 'zod'
import type { Track } from '@/lib/db'
import type { TrackListenStats } from '@/types'

export const ShuffleSchema = z.enum(['off', 'on'])
export const ShuffleModeSchema = z.enum([
  'random',
  'variety',
  'rediscover',
  'smart',
  'fresh',
  'duration',
])

export type ShuffleState = z.infer<typeof ShuffleSchema>
export type ShuffleMode = z.infer<typeof ShuffleModeSchema>
/** Fisher–Yates (or other) permutation — must preserve element identity. */
export type ShuffleAlgorithm = <T>(playlist: T[]) => T[]

/** Playlist membership paired with its library track (duplicates stay distinct). */
export interface PlaylistQueueEntry {
  track: Track
  sourceIndex: number
}

export interface ShuffleContext {
  statsByTrackId: ReadonlyMap<number, TrackListenStats>
  statsEnabled: boolean
  nowMs: number
  random?: () => number
}

/**
 * Checks if a value is a valid shuffle state.
 * @param value - The value to check.
 * @returns True if the value is a valid shuffle state, false otherwise.
 */
export function isShuffleState(value: unknown): value is ShuffleState {
  return ShuffleSchema.safeParse(value).success
}

export function isShuffleMode(value: unknown): value is ShuffleMode {
  return ShuffleModeSchema.safeParse(value).success
}
