import { describe, expect, it } from 'vitest'
import type { Track } from '@/lib/db'
import {
  appendToQueue,
  clearUpNext,
  enqueueNext,
  indexOfTrackOccurrence,
  isQueueIdle,
  jumpToQueueIndex,
  mapCursorAfterReorder,
  realignQueueAfterPlaylistReorder,
  remapSourceIndicesAfterReorder,
  removeFromQueue,
  reorderQueue,
  trackIdsForSaveScope,
  trackOccurrenceAtIndex,
  type QueueSnapshot,
} from './index'
import { resolvePlayingSourceIndex } from './playing-source-index'

function track(id: number): Track {
  return {
    id,
    tg_user_id: 1,
    file_id: `f${id}`,
    file_unique_id: `u${id}`,
    title: `Title ${id}`,
    performer: `Artist ${id}`,
    duration: id * 10,
    source: 'telegram',
    mime_type: 'audio/mpeg',
    file_size: 1000,
    created_at: '2024-01-01T00:00:00Z',
  }
}

function queue(ids: number[], cursor: number): QueueSnapshot {
  return { tracks: ids.map(track), cursor }
}

describe('isQueueIdle', () => {
  it('is idle when empty or cursor is negative', () => {
    expect(isQueueIdle({ tracks: [], cursor: -1 })).toBe(true)
    expect(isQueueIdle(queue([1], -1))).toBe(true)
    expect(isQueueIdle(queue([1], 0))).toBe(false)
  })
})

describe('enqueueNext', () => {
  it('starts playback when idle', () => {
    const result = enqueueNext({ tracks: [], cursor: -1 }, [track(1), track(2)])
    expect(result.tracks.map(t => t.id)).toEqual([1, 2])
    expect(result.cursor).toBe(0)
    expect(result.shouldPlay).toBe(true)
    expect(result.clearSource).toBe(true)
  })

  it('inserts after the cursor while playing', () => {
    const result = enqueueNext(queue([1, 2, 3], 0), [track(9)])
    expect(result.tracks.map(t => t.id)).toEqual([1, 9, 2, 3])
    expect(result.cursor).toBe(0)
    expect(result.nowPlayingChanged).toBe(false)
  })
})

describe('appendToQueue', () => {
  it('autoplays when idle', () => {
    const result = appendToQueue({ tracks: [], cursor: -1 }, [track(4)])
    expect(result.tracks.map(t => t.id)).toEqual([4])
    expect(result.cursor).toBe(0)
    expect(result.shouldPlay).toBe(true)
  })

  it('appends without moving the cursor while playing', () => {
    const result = appendToQueue(queue([1, 2], 1), [track(3), track(1)])
    expect(result.tracks.map(t => t.id)).toEqual([1, 2, 3, 1])
    expect(result.cursor).toBe(1)
    expect(result.nowPlayingChanged).toBe(false)
  })
})

describe('reorderQueue', () => {
  it('keeps the current entry playing when it is moved', () => {
    const result = reorderQueue(queue([1, 2, 3], 1), 1, 2)
    expect(result.tracks.map(t => t.id)).toEqual([1, 3, 2])
    expect(result.cursor).toBe(2)
    expect(result.clearSource).toBe(true)
  })

  it('adjusts the cursor when an earlier entry moves past it', () => {
    const result = reorderQueue(queue([1, 2, 3], 1), 0, 2)
    expect(result.tracks.map(t => t.id)).toEqual([2, 3, 1])
    expect(result.cursor).toBe(0)
  })

  it('keeps the playing duplicate slot when identical ids swap', () => {
    // B A1 A2 with A1 playing → drag A2 before A1 → B A2 A1, still on A1
    const result = reorderQueue(queue([2, 1, 1], 1), 2, 1)
    expect(result.tracks.map(t => t.id)).toEqual([2, 1, 1])
    expect(result.cursor).toBe(2)
  })
})

describe('mapCursorAfterReorder', () => {
  it('moves the cursor with the dragged playing entry', () => {
    expect(mapCursorAfterReorder(1, 1, 2)).toBe(2)
  })

  it('shifts the cursor when a later duplicate moves before it', () => {
    // B A1(playing) A2 → move A2 before A1
    expect(mapCursorAfterReorder(1, 2, 1)).toBe(2)
  })
})

describe('remapSourceIndicesAfterReorder', () => {
  it('keeps B highlighted after A B C → B A C while shuffled', () => {
    // Playlist A B C; B playing (membership 1). Shuffle queue may be [B, …]
    // with sourceIndices[cursor]=1. Drag A (0) to index 1 → B A C.
    const remapped = remapSourceIndicesAfterReorder([1, 0, 2], {
      fromIndex: 0,
      toIndex: 1,
    })
    expect(remapped?.[0]).toBe(0) // B's membership is now 0
    expect(resolvePlayingSourceIndex({
      currentTrackId: 2,
      playlistId: 7,
      playlistTrackIds: [2, 1, 3], // B A C
      queue: {
        cursor: 0,
        source: { type: 'playlist', playlistId: 7 },
        sourceIndices: remapped,
        trackIds: [2, 1, 3],
      },
    })).toBe(0)
  })

  it('moves the playing membership when that row itself is dragged', () => {
    // A B C, B at membership 1 dragged to 0 → B A C
    expect(remapSourceIndicesAfterReorder([1, 0, 2], {
      fromIndex: 1,
      toIndex: 0,
    })).toEqual([0, 1, 2])
  })
})

describe('realignQueueAfterPlaylistReorder', () => {
  it('follows the playing duplicate via drag indices when ids look unchanged', () => {
    const before = queue([2, 1, 1], 1)
    const afterIds = [2, 1, 1]
    const afterTracks = afterIds.map(track)
    const aligned = realignQueueAfterPlaylistReorder(before, afterTracks, {
      fromIndex: 2,
      toIndex: 1,
    })
    expect(aligned.cursor).toBe(2)
    expect(aligned.tracks.map(t => t.id)).toEqual([2, 1, 1])
  })

  it('moves the cursor when the playing duplicate itself is dragged', () => {
    // B A1(playing) A2 → drag A1 after A2
    const before = queue([2, 1, 1], 1)
    const aligned = realignQueueAfterPlaylistReorder(
      before,
      [2, 1, 1].map(track),
      { fromIndex: 1, toIndex: 2 },
    )
    expect(aligned.cursor).toBe(2)
  })

  it('does not use occurrence fallback when a move is provided', () => {
    // Occurrence fallback would keep cursor at 1 (first A); move requires 2.
    const before = queue([2, 1, 1], 1)
    const withoutMove = realignQueueAfterPlaylistReorder(
      before,
      [2, 1, 1].map(track),
    )
    expect(withoutMove.cursor).toBe(1)

    const withMove = realignQueueAfterPlaylistReorder(
      before,
      [2, 1, 1].map(track),
      { fromIndex: 2, toIndex: 1 },
    )
    expect(withMove.cursor).toBe(2)
  })
})

describe('removeFromQueue', () => {
  it('removes history without changing now playing', () => {
    const result = removeFromQueue(queue([1, 2, 3], 1), [0])
    expect(result.tracks.map(t => t.id)).toEqual([2, 3])
    expect(result.cursor).toBe(0)
    expect(result.nowPlayingChanged).toBe(false)
  })

  it('skips to the next entry when removing now playing', () => {
    const result = removeFromQueue(queue([1, 2, 3], 1), [1])
    expect(result.tracks.map(t => t.id)).toEqual([1, 3])
    expect(result.cursor).toBe(1)
    expect(result.nowPlayingChanged).toBe(true)
    expect(result.shouldPlay).toBe(true)
  })

  it('clears to idle when removing now playing with nothing after', () => {
    const result = removeFromQueue(queue([1, 2], 1), [1])
    expect(result.tracks).toEqual([])
    expect(result.cursor).toBe(-1)
    expect(result.shouldPlay).toBe(false)
  })
})

describe('jumpToQueueIndex', () => {
  it('moves the cursor without truncating the list', () => {
    const result = jumpToQueueIndex(queue([1, 2, 3, 4], 3), 1)
    expect(result.tracks.map(t => t.id)).toEqual([1, 2, 3, 4])
    expect(result.cursor).toBe(1)
    expect(result.clearSource).toBe(false)
  })
})

describe('clearUpNext', () => {
  it('drops entries after the cursor', () => {
    const result = clearUpNext(queue([1, 2, 3, 4], 1))
    expect(result.tracks.map(t => t.id)).toEqual([1, 2])
    expect(result.cursor).toBe(1)
    expect(result.clearSource).toBe(true)
  })
})

describe('trackIdsForSaveScope', () => {
  const q = queue([1, 2, 3, 4], 1)

  it('returns the full queue', () => {
    expect(trackIdsForSaveScope(q, 'full')).toEqual([1, 2, 3, 4])
  })

  it('returns from here', () => {
    expect(trackIdsForSaveScope(q, 'fromHere')).toEqual([2, 3, 4])
  })

  it('returns up next only', () => {
    expect(trackIdsForSaveScope(q, 'upNext')).toEqual([3, 4])
  })

  it('returns empty up next when nothing remains', () => {
    expect(trackIdsForSaveScope(queue([1, 2], 1), 'upNext')).toEqual([])
  })

  it('preserves duplicate ids', () => {
    expect(trackIdsForSaveScope(queue([1, 1, 2], 0), 'full')).toEqual([1, 1, 2])
  })
})

describe('trackOccurrenceAtIndex / indexOfTrackOccurrence', () => {
  it('counts earlier duplicates before the cursor', () => {
    expect(trackOccurrenceAtIndex([1, 2, 1], 0)).toBe(0)
    expect(trackOccurrenceAtIndex([1, 2, 1], 2)).toBe(1)
  })

  it('round-trips occurrence through a reordered list', () => {
    const before = [1, 2, 3]
    const cursor = 0
    const occurrence = trackOccurrenceAtIndex(before, cursor)
    const after = [2, 1, 3]
    expect(indexOfTrackOccurrence(after, before[cursor]!, occurrence)).toBe(1)
  })
})
