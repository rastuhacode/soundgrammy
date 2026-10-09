import type { Track } from '@/lib/db'

export type QueueSaveScope = 'full' | 'fromHere' | 'upNext'

export interface QueueSnapshot {
  tracks: Track[]
  cursor: number
}

export function isQueueIdle(queue: QueueSnapshot): boolean {
  return queue.tracks.length === 0 || queue.cursor < 0
}

/**
 * How many earlier queue/playlist slots share this index's track id
 * (0 = first occurrence). Distinguishes duplicate memberships.
 */
export function trackOccurrenceAtIndex(
  trackIds: readonly number[],
  index: number,
): number {
  const id = trackIds[index]
  if (id === undefined) return 0
  let occurrence = 0
  for (let i = 0; i < index; i++) {
    if (trackIds[i] === id) occurrence++
  }
  return occurrence
}

/** Index of the Nth occurrence of trackId (0-based), or -1. */
export function indexOfTrackOccurrence(
  trackIds: readonly number[],
  trackId: number,
  occurrence: number,
): number {
  let seen = 0
  for (let i = 0; i < trackIds.length; i++) {
    if (trackIds[i] === trackId) {
      if (seen === occurrence) return i
      seen++
    }
  }
  return -1
}

/** Track ids for a save-as-playlist scope (empty when the scope has nothing). */
export function trackIdsForSaveScope(
  queue: QueueSnapshot,
  scope: QueueSaveScope,
): number[] {
  if (isQueueIdle(queue) && scope !== 'full') {
    if (scope === 'upNext') return []
  }

  switch (scope) {
    case 'full':
      return queue.tracks.map(track => track.id)
    case 'fromHere':
      if (isQueueIdle(queue)) return []
      return queue.tracks.slice(queue.cursor).map(track => track.id)
    case 'upNext':
      if (isQueueIdle(queue) || queue.cursor >= queue.tracks.length - 1) return []
      return queue.tracks.slice(queue.cursor + 1).map(track => track.id)
  }
}
