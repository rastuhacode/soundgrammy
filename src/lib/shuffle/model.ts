import { z } from 'zod'

export const ShuffleSchema = z.enum(['off', 'on'])

export type ShuffleState = z.infer<typeof ShuffleSchema>
/** Fisher–Yates (or other) permutation — must preserve element identity. */
export type ShuffleAlgorithm = <T>(playlist: T[]) => T[]

/**
 * Checks if a value is a valid shuffle state.
 * @param value - The value to check.
 * @returns True if the value is a valid shuffle state, false otherwise.
 */
export function isShuffleState(value: unknown): value is ShuffleState {
  return ShuffleSchema.safeParse(value).success
}
