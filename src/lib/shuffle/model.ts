import type { Track } from '@/types'
import { z } from 'zod'

export const ShuffleSchema = z.enum(['off', 'on'])

export type ShuffleState = z.infer<typeof ShuffleSchema>
export type ShuffleAlgorithm = (playlist: Track[]) => Track[]

/**
 * Checks if a value is a valid shuffle state.
 * @param value - The value to check.
 * @returns True if the value is a valid shuffle state, false otherwise.
 */
export function isShuffleState(value: unknown): value is ShuffleState {
  return ShuffleSchema.safeParse(value).success
}
