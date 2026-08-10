import { fisherYates } from './default'
import type {
  PlaylistQueueEntry,
  ShuffleContext,
  ShuffleMode,
} from './model'

export interface ShuffleModeOption {
  id: ShuffleMode
  label: string
  description: string
}

export const SHUFFLE_MODE_OPTIONS: readonly ShuffleModeOption[] = [
  {
    id: 'random',
    label: 'Random',
    description: 'Gives every track an equal chance using a standard random shuffle.',
  },
  {
    id: 'variety',
    label: 'Variety',
    description: 'Spreads tracks by the same performer apart whenever possible.',
  },
  {
    id: 'rediscover',
    label: 'Rediscover',
    description: 'Prioritizes tracks you have never played or have not heard recently.',
  },
  {
    id: 'smart',
    label: 'Smart Mix',
    description: 'Balances favorites, forgotten tracks, discovery, and your skip history.',
  },
  {
    id: 'fresh',
    label: 'Fresh Mix',
    description: 'Favors recently added tracks while continuing to mix in older music.',
  },
  {
    id: 'duration',
    label: 'Duration Mix',
    description: 'Alternates shorter and longer tracks whenever the playlist allows it.',
  },
]

function weightedPermutation(
  entries: PlaylistQueueEntry[],
  weightFor: (entry: PlaylistQueueEntry) => number,
  random: () => number,
): PlaylistQueueEntry[] {
  return entries
    .map(entry => ({
      entry,
      // Exponential-race keys produce a weighted permutation without repeats.
      key: -Math.log(Math.max(Number.MIN_VALUE, random()))
        / Math.max(0.01, weightFor(entry)),
    }))
    .sort((a, b) => a.key - b.key)
    .map(item => item.entry)
}

function normalizedPerformer(entry: PlaylistQueueEntry): string | null {
  const performer = entry.track.performer?.trim().toLocaleLowerCase()
  return performer || null
}

function varietyShuffle(
  entries: PlaylistQueueEntry[],
  random: () => number,
): PlaylistQueueEntry[] {
  const remaining = fisherYates(entries, random)
  const result: PlaylistQueueEntry[] = []
  const recentPerformers: string[] = []
  const spacing = 3

  while (remaining.length > 0) {
    const eligible = remaining
      .map((entry, index) => ({ entry, index }))
      .filter(({ entry }) => {
        const performer = normalizedPerformer(entry)
        return performer === null || !recentPerformers.includes(performer)
      })
    const candidates = eligible.length > 0
      ? eligible
      : remaining.map((entry, index) => ({ entry, index }))
    const selected = candidates[Math.floor(random() * candidates.length)]!
    const [entry] = remaining.splice(selected.index, 1)
    result.push(entry!)

    const performer = normalizedPerformer(entry!)
    if (performer !== null) {
      recentPerformers.push(performer)
      if (recentPerformers.length > spacing) recentPerformers.shift()
    }
  }

  return result
}

function daysSince(timestamp: number | null, nowMs: number): number | null {
  if (timestamp === null) return null
  return Math.max(0, nowMs - timestamp) / 86_400_000
}

function rediscoverShuffle(
  entries: PlaylistQueueEntry[],
  context: ShuffleContext,
  random: () => number,
): PlaylistQueueEntry[] {
  if (!context.statsEnabled) return fisherYates(entries, random)
  return weightedPermutation(entries, (entry) => {
    const stats = context.statsByTrackId.get(entry.track.id)
    if (!stats || stats.starts === 0) return 8
    const staleDays = daysSince(stats.last_played_at_ms, context.nowMs) ?? 365
    return 1 + Math.min(7, staleDays / 30) + 1 / Math.sqrt(stats.starts)
  }, random)
}

function smartShuffle(
  entries: PlaylistQueueEntry[],
  context: ShuffleContext,
  random: () => number,
): PlaylistQueueEntry[] {
  if (!context.statsEnabled) return fisherYates(entries, random)
  return weightedPermutation(entries, (entry) => {
    const stats = context.statsByTrackId.get(entry.track.id)
    if (!stats || stats.starts === 0) return 3.5
    const staleDays = daysSince(stats.last_played_at_ms, context.nowMs) ?? 365
    const earlySkipRate = stats.early_skips / Math.max(1, stats.starts)
    return Math.max(
      0.15,
      1
      + Math.log1p(stats.likeness)
      + Math.min(2, staleDays / 90)
      - 2 * earlySkipRate,
    )
  }, random)
}

function freshShuffle(
  entries: PlaylistQueueEntry[],
  context: ShuffleContext,
  random: () => number,
): PlaylistQueueEntry[] {
  return weightedPermutation(entries, (entry) => {
    const addedAt = Date.parse(entry.track.created_at)
    if (!Number.isFinite(addedAt)) return 1
    const ageDays = Math.max(0, context.nowMs - addedAt) / 86_400_000
    return 1 + 4 * Math.exp(-ageDays / 30)
  }, random)
}

function durationShuffle(
  entries: PlaylistQueueEntry[],
  random: () => number,
): PlaylistQueueEntry[] {
  const shuffled = fisherYates(entries, random)
  const durations = shuffled
    .map(entry => entry.track.duration)
    .filter((duration): duration is number => duration != null && duration > 0)
    .sort((a, b) => a - b)
  if (durations.length < 2) return shuffled

  const median = durations[Math.floor(durations.length / 2)]!
  const shorter = shuffled.filter(entry =>
    entry.track.duration != null
    && entry.track.duration > 0
    && entry.track.duration <= median,
  )
  const longer = shuffled.filter(entry =>
    entry.track.duration != null && entry.track.duration > median,
  )
  const unknown = shuffled.filter(entry =>
    entry.track.duration == null || entry.track.duration <= 0,
  )
  if (shorter.length === 0 || longer.length === 0) return shuffled

  const result: PlaylistQueueEntry[] = []
  let takeShorter = shorter.length >= longer.length
  while (shorter.length > 0 || longer.length > 0) {
    const preferred = takeShorter ? shorter : longer
    const fallback = takeShorter ? longer : shorter
    result.push((preferred.shift() ?? fallback.shift())!)
    takeShorter = !takeShorter
  }
  for (const entry of unknown) {
    result.splice(Math.floor(random() * (result.length + 1)), 0, entry)
  }
  return result
}

export function shuffleEntriesByMode(
  entries: PlaylistQueueEntry[],
  mode: ShuffleMode,
  context: ShuffleContext,
): PlaylistQueueEntry[] {
  const random = context.random ?? Math.random
  switch (mode) {
    case 'random': return fisherYates(entries, random)
    case 'variety': return varietyShuffle(entries, random)
    case 'rediscover': return rediscoverShuffle(entries, context, random)
    case 'smart': return smartShuffle(entries, context, random)
    case 'fresh': return freshShuffle(entries, context, random)
    case 'duration': return durationShuffle(entries, random)
  }
}
