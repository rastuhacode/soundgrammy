import { describe, expect, it } from 'vitest'
import type { DownloadProgress } from '@/lib/api'
import {
  computeBufferedRanges,
  landTimeOnRanges,
  snapTimeToRanges,
  timeIsInRanges,
  toCachedRanges,
} from './buffer-ranges'

function progress(
  partial: Partial<DownloadProgress> & Pick<DownloadProgress, 'ranges'>,
): DownloadProgress {
  return {
    trackId: 1,
    received: 0,
    total: 1000,
    complete: false,
    ...partial,
  }
}

describe('toCachedRanges', () => {
  it('maps byte ranges onto the media timeline', () => {
    expect(toCachedRanges(
      progress({ ranges: [{ start: 0, end: 250 }], total: 1000 }),
      200,
    )).toEqual([{ start: 0, end: 50 }])
  })
})

describe('timeIsInRanges', () => {
  it('detects membership in half-open ranges', () => {
    expect(timeIsInRanges(15, [{ start: 10, end: 20 }])).toBe(true)
    expect(timeIsInRanges(20, [{ start: 10, end: 20 }])).toBe(false)
  })
})

describe('snapTimeToRanges', () => {
  it('returns the time when already inside a range', () => {
    expect(snapTimeToRanges(12, [{ start: 10, end: 20 }])).toBe(12)
  })

  it('snaps slightly-before targets onto the island start', () => {
    expect(snapTimeToRanges(9.2, [{ start: 10, end: 13 }])).toBe(10)
  })

  it('returns null when no island is near enough', () => {
    expect(snapTimeToRanges(10, [{ start: 120, end: 125 }])).toBeNull()
  })
})

describe('landTimeOnRanges', () => {
  it('lands on a nearby island after discontinuous seek', () => {
    expect(landTimeOnRanges(180, [{ start: 170, end: 175 }])).toBe(174.95)
  })

  it('does not snap an earlier scrub onto a stale later island', () => {
    expect(landTimeOnRanges(30, [{ start: 120, end: 125 }])).toBeNull()
  })
})

describe('computeBufferedRanges', () => {
  const duration = 200

  it('prefers the active media buffered island around the playhead', () => {
    expect(computeBufferedRanges({
      mediaRanges: [
        { start: 0, end: 30 },
        { start: 80, end: 110 },
      ],
      playableEnd: 110,
      duration,
      currentTime: 90,
    })).toEqual([{ start: 80, end: 110 }])
  })

  it('falls back to playableEnd when media ranges are empty', () => {
    expect(computeBufferedRanges({
      playableEnd: 60,
      duration,
      cachedRanges: [{ start: 0, end: 120 }],
    })).toEqual([{ start: 0, end: 60 }])
  })

  it('falls back to the leading download prefix before any append', () => {
    expect(computeBufferedRanges({
      playableEnd: 0,
      duration,
      cachedRanges: [{ start: 0, end: 40 }],
    })).toEqual([{ start: 0, end: 40 }])
  })
})
