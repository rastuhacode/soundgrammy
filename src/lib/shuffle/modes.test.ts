import { describe, expect, it } from 'vitest'
import type { Track } from '@/lib/db'
import type { TrackListenStats } from '@/types'
import { buildPlaylistEntries } from './index'
import { fisherYates } from './default'
import { SHUFFLE_MODE_OPTIONS, shuffleEntriesByMode } from './modes'
import type { ShuffleContext, ShuffleMode } from './model'

const NOW = Date.parse('2026-08-10T12:00:00Z')

function track(
  id: number,
  options: Partial<Pick<Track, 'performer' | 'duration' | 'created_at'>> = {},
): Track {
  return {
    id,
    tg_user_id: 1,
    file_id: `f${id}`,
    file_unique_id: `u${id}`,
    title: `Title ${id}`,
    performer: options.performer ?? `Artist ${id}`,
    duration: options.duration ?? id * 60,
    source: 'telegram',
    mime_type: 'audio/mpeg',
    file_size: 1000,
    created_at: options.created_at ?? '2025-01-01T00:00:00Z',
  }
}

function stats(
  trackId: number,
  options: Partial<TrackListenStats> = {},
): TrackListenStats {
  return {
    track_id: trackId,
    starts: 1,
    qualified_plays: 1,
    completes: 0,
    early_skips: 0,
    total_listened_ms: 60_000,
    first_played_at_ms: NOW - 86_400_000,
    last_played_at_ms: NOW - 86_400_000,
    likeness: 1,
    ...options,
  }
}

function seededRandom(seed: number): () => number {
  let state = seed >>> 0
  return () => {
    state = (Math.imul(1_664_525, state) + 1_013_904_223) >>> 0
    return state / 2 ** 32
  }
}

function context(
  options: Partial<ShuffleContext> = {},
): ShuffleContext {
  return {
    statsByTrackId: new Map(),
    statsEnabled: true,
    nowMs: NOW,
    random: seededRandom(1),
    ...options,
  }
}

describe('shuffle modes', () => {
  it('exposes every requested mode and no daily mode', () => {
    expect(SHUFFLE_MODE_OPTIONS.map(option => option.id)).toEqual([
      'random',
      'variety',
      'rediscover',
      'smart',
      'fresh',
      'duration',
    ])
  })

  it.each(SHUFFLE_MODE_OPTIONS.map(option => option.id))(
    '%s preserves every playlist membership exactly once',
    (mode: ShuffleMode) => {
      const entries = buildPlaylistEntries([
        track(1),
        track(1),
        track(2),
        track(3),
      ])
      const shuffled = shuffleEntriesByMode(entries, mode, context())
      expect(shuffled).not.toBe(entries)
      expect(shuffled.map(entry => entry.sourceIndex).sort((a, b) => a - b))
        .toEqual([0, 1, 2, 3])
    },
  )

  it('variety spaces the same performer apart when alternatives exist', () => {
    const entries = buildPlaylistEntries([
      track(1, { performer: 'Same' }),
      track(2, { performer: 'Other 1' }),
      track(3, { performer: 'Other 2' }),
      track(4, { performer: 'Other 3' }),
      track(5, { performer: 'Other 4' }),
      track(6, { performer: 'Same' }),
    ])
    const shuffled = shuffleEntriesByMode(
      entries,
      'variety',
      context({ random: seededRandom(7) }),
    )
    const samePositions = shuffled
      .map((entry, index) => entry.track.performer === 'Same' ? index : -1)
      .filter(index => index >= 0)
    expect(Math.abs(samePositions[1]! - samePositions[0]!)).toBeGreaterThan(3)
  })

  it('listen-based modes use ordinary random order when statistics are disabled', () => {
    const entries = buildPlaylistEntries([track(1), track(2), track(3), track(4)])
    for (const mode of ['rediscover', 'smart'] as const) {
      const actual = shuffleEntriesByMode(
        entries,
        mode,
        context({ statsEnabled: false, random: seededRandom(9) }),
      )
      const expected = fisherYates(entries, seededRandom(9))
      expect(actual).toEqual(expected)
    }
  })

  it('rediscover strongly favors an unplayed track over a recently played one', () => {
    const entries = buildPlaylistEntries([track(1), track(2)])
    const byId = new Map([
      [2, stats(2, { starts: 20, last_played_at_ms: NOW })],
    ])
    let unplayedFirst = 0
    for (let seed = 1; seed <= 100; seed++) {
      const result = shuffleEntriesByMode(
        entries,
        'rediscover',
        context({ statsByTrackId: byId, random: seededRandom(seed) }),
      )
      if (result[0]?.track.id === 1) unplayedFirst++
    }
    expect(unplayedFirst).toBeGreaterThan(75)
  })

  it('smart mix downweights a repeatedly skipped track', () => {
    const entries = buildPlaylistEntries([track(1), track(2)])
    const byId = new Map([
      [1, stats(1, { starts: 10, likeness: 5 })],
      [2, stats(2, { starts: 10, early_skips: 10, likeness: 0 })],
    ])
    let likedFirst = 0
    for (let seed = 1; seed <= 100; seed++) {
      const result = shuffleEntriesByMode(
        entries,
        'smart',
        context({ statsByTrackId: byId, random: seededRandom(seed) }),
      )
      if (result[0]?.track.id === 1) likedFirst++
    }
    expect(likedFirst).toBeGreaterThan(80)
  })

  it('fresh mix favors a recently added track without excluding old tracks', () => {
    const entries = buildPlaylistEntries([
      track(1, { created_at: '2026-08-10T00:00:00Z' }),
      track(2, { created_at: '2020-01-01T00:00:00Z' }),
    ])
    let freshFirst = 0
    for (let seed = 1; seed <= 100; seed++) {
      const result = shuffleEntriesByMode(
        entries,
        'fresh',
        context({ random: seededRandom(seed) }),
      )
      if (result[0]?.track.id === 1) freshFirst++
      expect(result).toHaveLength(2)
    }
    expect(freshFirst).toBeGreaterThan(70)
  })

  it('duration mix alternates the shorter and longer halves while possible', () => {
    const durations = [60, 70, 80, 90, 300, 310, 320, 330]
    const entries = buildPlaylistEntries(
      durations.map((duration, index) => track(index + 1, { duration })),
    )
    const shuffled = shuffleEntriesByMode(
      entries,
      'duration',
      context({ random: seededRandom(3) }),
    )
    const groups = shuffled.slice(0, 6).map(entry =>
      entry.track.duration! <= 300 ? 'short' : 'long',
    )
    expect(groups).toEqual(['short', 'long', 'short', 'long', 'short', 'long'])
  })
})
