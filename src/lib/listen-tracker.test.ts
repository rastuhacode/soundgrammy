import { describe, expect, it } from 'vitest'
import {
  clearPendingListenEndReason,
  clockListenedMs,
  clockOnPause,
  clockOnPlay,
  createAttemptClock,
  effectiveListenedMs,
  lastFmThresholdMs,
  setPendingListenEndReason,
  takePendingListenEndReason,
  trackDurationMs,
} from './listen-tracker'

describe('trackDurationMs', () => {
  it('converts seconds to ms', () => {
    expect(trackDurationMs(180)).toBe(180_000)
  })

  it('returns null for missing or non-positive', () => {
    expect(trackDurationMs(null)).toBeNull()
    expect(trackDurationMs(0)).toBeNull()
    expect(trackDurationMs(-1)).toBeNull()
  })
})

describe('effectiveListenedMs', () => {
  it('caps at duration for non-complete', () => {
    expect(effectiveListenedMs(90_000, 60_000, 'skipped')).toBe(60_000)
  })

  it('uses full duration on completed', () => {
    expect(effectiveListenedMs(5_000, 180_000, 'completed')).toBe(180_000)
  })

  it('keeps raw L when duration unknown', () => {
    expect(effectiveListenedMs(12_000, null, 'stopped')).toBe(12_000)
  })
})

describe('lastFmThresholdMs', () => {
  it('rejects tracks of 30 seconds or less', () => {
    expect(lastFmThresholdMs(30)).toBeNull()
  })

  it('qualifies a 31-second track at half and caps long tracks at four minutes', () => {
    expect(lastFmThresholdMs(31)).toBe(15_500)
    expect(lastFmThresholdMs(900)).toBe(240_000)
  })

  it('uses four minutes for unknown duration', () => {
    expect(lastFmThresholdMs(null)).toBe(240_000)
  })
})

describe('attempt clock', () => {
  it('accumulates only while playing', () => {
    let clock = createAttemptClock()
    clock = clockOnPlay(clock, 1000)
    clock = clockOnPause(clock, 1500)
    expect(clockListenedMs(clock, 2000)).toBe(500)
    clock = clockOnPlay(clock, 2000)
    expect(clockListenedMs(clock, 2300)).toBe(800)
  })
})

describe('pending end reason', () => {
  it('takes and clears', () => {
    clearPendingListenEndReason()
    setPendingListenEndReason('skipped')
    expect(takePendingListenEndReason()).toBe('skipped')
    expect(takePendingListenEndReason('replaced')).toBe('replaced')
  })
})
