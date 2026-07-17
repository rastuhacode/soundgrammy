import type { Track } from '@/lib/db'

export type QueueSaveScope = 'full' | 'fromHere' | 'upNext'

export interface QueueSnapshot {
  tracks: Track[]
  cursor: number
}

export interface QueueMutationResult extends QueueSnapshot {
  /** Whether now-playing identity changed (for listen-tracker bookkeeping). */
  nowPlayingChanged: boolean
  /** True when the queue should start/continue playing after this mutation. */
  shouldPlay: boolean
  /** Cleared when the queue was materially edited. */
  clearSource: boolean
}

function normalizeCursor(tracks: Track[], cursor: number): number {
  if (tracks.length === 0) return -1
  return Math.min(Math.max(cursor, 0), tracks.length - 1)
}

export function isQueueIdle(queue: QueueSnapshot): boolean {
  return queue.tracks.length === 0 || queue.cursor < 0
}

/** Insert tracks immediately after the cursor. Idle → start at first inserted. */
export function enqueueNext(
  queue: QueueSnapshot,
  tracks: Track[],
): QueueMutationResult {
  if (tracks.length === 0) {
    return {
      ...queue,
      nowPlayingChanged: false,
      shouldPlay: !isQueueIdle(queue),
      clearSource: false,
    }
  }

  if (isQueueIdle(queue)) {
    return {
      tracks: [...tracks],
      cursor: 0,
      nowPlayingChanged: true,
      shouldPlay: true,
      clearSource: true,
    }
  }

  const insertAt = queue.cursor + 1
  const nextTracks = [
    ...queue.tracks.slice(0, insertAt),
    ...tracks,
    ...queue.tracks.slice(insertAt),
  ]
  return {
    tracks: nextTracks,
    cursor: queue.cursor,
    nowPlayingChanged: false,
    shouldPlay: true,
    clearSource: true,
  }
}

/** Append tracks. Idle → autoplay; playing → append only. */
export function appendToQueue(
  queue: QueueSnapshot,
  tracks: Track[],
): QueueMutationResult {
  if (tracks.length === 0) {
    return {
      ...queue,
      nowPlayingChanged: false,
      shouldPlay: !isQueueIdle(queue),
      clearSource: false,
    }
  }

  if (isQueueIdle(queue)) {
    return {
      tracks: [...tracks],
      cursor: 0,
      nowPlayingChanged: true,
      shouldPlay: true,
      clearSource: true,
    }
  }

  return {
    tracks: [...queue.tracks, ...tracks],
    cursor: queue.cursor,
    nowPlayingChanged: false,
    shouldPlay: true,
    clearSource: true,
  }
}

/**
 * Move an entry from `fromIndex` to `toIndex`.
 * Keeps the same queue entry as now playing when possible.
 */
export function mapCursorAfterReorder(
  cursor: number,
  fromIndex: number,
  toIndex: number,
): number {
  if (fromIndex === toIndex || cursor < 0) return cursor
  if (cursor === fromIndex) return toIndex
  if (fromIndex < toIndex) {
    // Moved down: slots in (fromIndex, toIndex] shift left.
    if (cursor > fromIndex && cursor <= toIndex) return cursor - 1
  }
  else {
    // Moved up: slots in [toIndex, fromIndex) shift right.
    if (cursor >= toIndex && cursor < fromIndex) return cursor + 1
  }
  return cursor
}

/**
 * After a playlist membership drag, remap each queue slot's sourceIndex so
 * shuffled sessions still highlight the same membership (not the old index).
 */
export function remapSourceIndicesAfterReorder(
  sourceIndices: number[] | null | undefined,
  move: { fromIndex: number, toIndex: number },
): number[] | null {
  if (!sourceIndices) return null
  return sourceIndices.map(sourceIndex =>
    mapCursorAfterReorder(sourceIndex, move.fromIndex, move.toIndex),
  )
}

/**
 * Move an entry from `fromIndex` to `toIndex`.
 * Keeps the same queue entry as now playing when possible.
 */
export function reorderQueue(
  queue: QueueSnapshot,
  fromIndex: number,
  toIndex: number,
): QueueMutationResult {
  if (
    fromIndex < 0
    || toIndex < 0
    || fromIndex >= queue.tracks.length
    || toIndex >= queue.tracks.length
    || fromIndex === toIndex
  ) {
    return {
      ...queue,
      nowPlayingChanged: false,
      shouldPlay: !isQueueIdle(queue),
      clearSource: false,
    }
  }

  const nextTracks = [...queue.tracks]
  const [moved] = nextTracks.splice(fromIndex, 1)
  nextTracks.splice(toIndex, 0, moved!)

  return {
    tracks: nextTracks,
    cursor: normalizeCursor(
      nextTracks,
      mapCursorAfterReorder(queue.cursor, fromIndex, toIndex),
    ),
    nowPlayingChanged: false,
    shouldPlay: true,
    clearSource: true,
  }
}

/**
 * Rebuild playlist-sourced queue order after a membership drag.
 * Prefer `move` so duplicate track ids remapping stays index-accurate.
 */
export function realignQueueAfterPlaylistReorder(
  queue: QueueSnapshot,
  nextTracks: Track[],
  move?: { fromIndex: number, toIndex: number },
): QueueSnapshot {
  if (nextTracks.length === 0) {
    return { tracks: [], cursor: -1 }
  }

  let nextCursor = queue.cursor
  if (move) {
    nextCursor = mapCursorAfterReorder(
      queue.cursor,
      move.fromIndex,
      move.toIndex,
    )
  }
  else if (queue.cursor >= 0) {
    const playingId = queue.tracks[queue.cursor]?.id
    if (playingId != null) {
      const occurrence = trackOccurrenceAtIndex(
        queue.tracks.map(track => track.id),
        queue.cursor,
      )
      nextCursor = indexOfTrackOccurrence(
        nextTracks.map(track => track.id),
        playingId,
        occurrence,
      )
      if (nextCursor === -1) {
        nextCursor = nextTracks.findIndex(track => track.id === playingId)
      }
    }
  }

  return {
    tracks: [...nextTracks],
    cursor: normalizeCursor(nextTracks, nextCursor),
  }
}

/**
 * Remove entries at the given indices (deduped, highest first).
 * Removing now playing advances like skip, or clears to idle when nothing remains.
 */
export function removeFromQueue(
  queue: QueueSnapshot,
  indices: number[],
): QueueMutationResult {
  const unique = [...new Set(indices)]
    .filter(index => index >= 0 && index < queue.tracks.length)
    .sort((a, b) => b - a)

  if (unique.length === 0) {
    return {
      ...queue,
      nowPlayingChanged: false,
      shouldPlay: !isQueueIdle(queue),
      clearSource: false,
    }
  }

  let tracks = [...queue.tracks]
  let cursor = queue.cursor
  let removedNowPlaying = false

  for (const index of unique) {
    if (index === cursor) removedNowPlaying = true
    tracks.splice(index, 1)
    if (index < cursor) cursor -= 1
    else if (index === cursor) {
      // Cursor stays on the next entry (same index after splice).
    }
  }

  if (tracks.length === 0) {
    return {
      tracks: [],
      cursor: -1,
      nowPlayingChanged: true,
      shouldPlay: false,
      clearSource: true,
    }
  }

  if (removedNowPlaying) {
    if (cursor >= tracks.length) {
      // No next entry after removing now playing — clear to idle.
      return {
        tracks: [],
        cursor: -1,
        nowPlayingChanged: true,
        shouldPlay: false,
        clearSource: true,
      }
    }
    return {
      tracks,
      cursor,
      nowPlayingChanged: true,
      shouldPlay: true,
      clearSource: true,
    }
  }

  return {
    tracks,
    cursor: normalizeCursor(tracks, cursor),
    nowPlayingChanged: false,
    shouldPlay: true,
    clearSource: true,
  }
}

export function jumpToQueueIndex(
  queue: QueueSnapshot,
  index: number,
): QueueMutationResult {
  if (queue.tracks.length === 0) {
    return {
      tracks: [],
      cursor: -1,
      nowPlayingChanged: false,
      shouldPlay: false,
      clearSource: false,
    }
  }

  const cursor = normalizeCursor(queue.tracks, index)
  return {
    tracks: queue.tracks,
    cursor,
    nowPlayingChanged: cursor !== queue.cursor,
    shouldPlay: true,
    clearSource: false,
  }
}

export function clearUpNext(queue: QueueSnapshot): QueueMutationResult {
  if (isQueueIdle(queue) || queue.cursor >= queue.tracks.length - 1) {
    return {
      ...queue,
      nowPlayingChanged: false,
      shouldPlay: !isQueueIdle(queue),
      clearSource: false,
    }
  }

  return {
    tracks: queue.tracks.slice(0, queue.cursor + 1),
    cursor: queue.cursor,
    nowPlayingChanged: false,
    shouldPlay: true,
    clearSource: true,
  }
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
