import { describe, expect, it } from 'vitest'
import type { Track } from '@/lib/db'
import {
  buildPlaylistEntries,
  shufflePlaylistEntries,
} from './index'

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

/** Deterministic "shuffle": reverse order (keeps entry identity). */
function reverseShuffle<T>(items: T[]): T[] {
  return [...items].reverse()
}

describe('shufflePlaylistEntries', () => {
  it('pins by sourceIndex so duplicate track ids stay distinct', () => {
    // Playlist: A1 A2 B — start from A2 (sourceIndex 1)
    const entries = buildPlaylistEntries([track(1), track(1), track(2)])
    const shuffled = shufflePlaylistEntries(entries, reverseShuffle, 1)

    expect(shuffled[0]?.sourceIndex).toBe(1)
    expect(shuffled[0]?.track.id).toBe(1)
    // Remaining memberships keep their sourceIndex identity
    expect(shuffled.map(entry => entry.sourceIndex).sort()).toEqual([0, 1, 2])
  })

  it('does not pin the first duplicate when a later membership is requested', () => {
    const entries = buildPlaylistEntries([track(1), track(1), track(2)])
    // reverse → [B, A2, A1]; pin sourceIndex 1 (A2) → [A2, B, A1]
    const shuffled = shufflePlaylistEntries(entries, reverseShuffle, 1)
    expect(shuffled.map(entry => entry.sourceIndex)).toEqual([1, 2, 0])
  })

  it('leaves a single-entry list unchanged', () => {
    const entries = buildPlaylistEntries([track(1)])
    expect(shufflePlaylistEntries(entries, reverseShuffle, 0)).toEqual(entries)
  })
})
