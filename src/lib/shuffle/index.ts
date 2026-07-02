import type { Track } from '@/lib/db'
import type { ShuffleAlgorithm } from './model'

export type { ShuffleState, ShuffleAlgorithm } from './model'
export { isShuffleState } from './model'

/**
 * Applies the shuffle algorithm to the tracks, pins the track with the given ID at the beginning.
 * @param original - The orifinal array of tracks to shuffle.
 * @param algorithm - The shuffle algorithm to use.
 * @param pinnedId - The ID of the pinned tarck to keep at the beginning.
 * @returns The new shuffled tracks array.
 */
export function applyAlgorithm(
  original: Track[],
  algorithm: ShuffleAlgorithm,
  pinnedId?: Track['id'],
): Track[] {
  if (original.length <= 1) return original

  let shuffled = algorithm(original)
  if (pinnedId === undefined) return shuffled

  const pinIndex = shuffled.findIndex(track => track.id === pinnedId)
  if (pinIndex > 0) {
    const [pinned] = shuffled.splice(pinIndex, 1)
    shuffled = [pinned!, ...shuffled]
  }

  return shuffled
}
