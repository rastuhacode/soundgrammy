import type { Track } from '@/lib/db'
import { shuffleEntriesByMode } from './modes'
import type {
  PlaylistQueueEntry,
  ShuffleAlgorithm,
  ShuffleContext,
  ShuffleMode,
} from './model'

export type {
  PlaylistQueueEntry,
  ShuffleAlgorithm,
  ShuffleContext,
  ShuffleMode,
  ShuffleState,
} from './model'
export { isShuffleMode, isShuffleState } from './model'
export { SHUFFLE_MODE_OPTIONS } from './modes'

export function buildPlaylistEntries(tracks: Track[]): PlaylistQueueEntry[] {
  return tracks.map((track, sourceIndex) => ({ track, sourceIndex }))
}

/**
 * Applies the shuffle algorithm to the tracks, pins the track with the given ID at the beginning.
 * @param original - The original array of tracks to shuffle.
 * @param algorithm - The shuffle algorithm to use.
 * @param pinnedId - The ID of the pinned track to keep at the beginning.
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

/**
 * Shuffle playlist memberships while preserving which duplicate slot is which.
 * Pins by `sourceIndex` (not track id) so A1 vs A2 stay distinct.
 */
export function shufflePlaylistEntries(
  entries: PlaylistQueueEntry[],
  algorithm: ShuffleAlgorithm,
  pinSourceIndex?: number,
): PlaylistQueueEntry[] {
  if (entries.length <= 1) return entries

  const shuffled = algorithm(entries)
  if (pinSourceIndex === undefined) return shuffled

  const pinIndex = shuffled.findIndex(entry => entry.sourceIndex === pinSourceIndex)
  if (pinIndex > 0) {
    const next = [...shuffled]
    const [pinned] = next.splice(pinIndex, 1)
    return [pinned!, ...next]
  }
  return shuffled
}

export function shufflePlaylistEntriesByMode(
  entries: PlaylistQueueEntry[],
  mode: ShuffleMode,
  context: ShuffleContext,
  pinSourceIndex?: number,
): PlaylistQueueEntry[] {
  if (entries.length <= 1) return entries

  const shuffled = shuffleEntriesByMode(entries, mode, context)
  if (pinSourceIndex === undefined) return shuffled

  const pinIndex = shuffled.findIndex(entry => entry.sourceIndex === pinSourceIndex)
  if (pinIndex > 0) {
    const next = [...shuffled]
    const [pinned] = next.splice(pinIndex, 1)
    return [pinned!, ...next]
  }
  return shuffled
}
