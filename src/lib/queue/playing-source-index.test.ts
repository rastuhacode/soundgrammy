import { describe, expect, it } from 'vitest'
import { resolvePlayingSourceIndex } from './playing-source-index'
import { buildPlaylistEntries, shufflePlaylistEntries } from '@/lib/shuffle'
import type { Track } from '@/lib/db'

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

function reverseShuffle<T>(items: T[]): T[] {
  return [...items].reverse()
}

describe('resolvePlayingSourceIndex', () => {
  const playlistId = 42
  const playlistTrackIds = [1, 1, 2] // A1 A2 B

  it('uses sourceIndices under shuffle so A2 stays highlighted', () => {
    // Start A2 (index 1), shuffle pins that membership to cursor 0
    const entries = shufflePlaylistEntries(
      buildPlaylistEntries([track(1), track(1), track(2)]),
      reverseShuffle,
      1,
    )
    expect(resolvePlayingSourceIndex({
      currentTrackId: 1,
      playlistId,
      playlistTrackIds,
      queue: {
        cursor: 0,
        source: { type: 'playlist', playlistId },
        sourceIndices: entries.map(entry => entry.sourceIndex),
        trackIds: entries.map(entry => entry.track.id),
      },
    })).toBe(1)
  })

  it('does not fall back to the first duplicate when sourceIndices are present', () => {
    expect(resolvePlayingSourceIndex({
      currentTrackId: 1,
      playlistId,
      playlistTrackIds,
      queue: {
        cursor: 0,
        source: { type: 'playlist', playlistId },
        sourceIndices: [1, 0, 2],
        trackIds: [1, 1, 2],
      },
    })).toBe(1)
  })

  it('keeps the second duplicate highlighted after source is cleared (play next)', () => {
    // Playing A2, then play-next inserts X after cursor and clears source.
    expect(resolvePlayingSourceIndex({
      currentTrackId: 1,
      playlistId,
      playlistTrackIds,
      queue: {
        cursor: 1,
        source: null,
        sourceIndices: null,
        trackIds: [1, 1, 9, 2],
      },
    })).toBe(1)
  })
})
