import { z } from 'zod'

export const RepeatSchema = z.enum(['none', 'one', 'all'])
export type RepeatState = z.infer<typeof RepeatSchema>

export function isRepeatState(value: unknown): value is RepeatState {
  return RepeatSchema.safeParse(value).success
}
