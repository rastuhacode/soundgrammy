import { describe, expect, it } from 'vitest'
import type { Track } from '@/lib/db'
import {
  buildPlaylistEntries,
  shufflePlaylistEntries,
} from '@/lib/shuffle'

function track(id: number, title: string): Track {
  return {
    id,
    tg_user_id: 1,
    file_id: `f${id}`,
    file_unique_id: `u${id}`,
    title,
    performer: 'Artist',
    duration: 100,
    source: 'telegram',
    mime_type: 'audio/mpeg',
    file_size: 1000,
    created_at: '2024-01-01T00:00:00Z',
  }
}

/** Mirrors player-store shuffle-off restore using baseEntries. */
function restoreFromBase(
  baseEntries: ReturnType<typeof buildPlaylistEntries>,
  playingSourceIndex: number,
) {
  const sourceIndices = baseEntries.map(entry => entry.sourceIndex)
  const mapped = sourceIndices.indexOf(playingSourceIndex)
  return {
    trackIds: baseEntries.map(entry => entry.track.id),
    cursor: mapped === -1 ? 0 : mapped,
    sourceIndices,
  }
}

describe('shuffle on/off with sorted baseEntries', () => {
  it('restores sorted session order after shuffle off', () => {
    // Membership C,A,B — UI sort by title → A,B,C with sourceIndexes 1,2,0
    const membership = [track(3, 'C'), track(1, 'A'), track(2, 'B')]
    const sorted = [
      { track: membership[1]!, sourceIndex: 1 },
      { track: membership[2]!, sourceIndex: 2 },
      { track: membership[0]!, sourceIndex: 0 },
    ]

    const shuffled = shufflePlaylistEntries(sorted, items => [...items].reverse(), 1)
    expect(shuffled[0]?.sourceIndex).toBe(1)

    const restored = restoreFromBase(sorted, 1)
    expect(restored.trackIds).toEqual([1, 2, 3])
    expect(restored.cursor).toBe(0)
    expect(restored.sourceIndices).toEqual([1, 2, 0])
  })
})
