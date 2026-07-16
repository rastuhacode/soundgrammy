import { describe, expect, it } from 'vitest'
import {
  appendedEndTime,
  leadingPrefixEnd,
  nextAppendWindow,
  shouldEndOfStream,
  shouldRebuildFromPrefix,
} from './mse-append-queue'

describe('leadingPrefixEnd', () => {
  it('returns the contiguous prefix ending at byte 0', () => {
    expect(leadingPrefixEnd([
      { start: 0, end: 128_000 },
      { start: 256_000, end: 384_000 },
    ])).toBe(128_000)
  })

  it('returns 0 when there is no leading range', () => {
    expect(leadingPrefixEnd([{ start: 128_000, end: 256_000 }])).toBe(0)
    expect(leadingPrefixEnd([])).toBe(0)
  })
})

describe('shouldRebuildFromPrefix', () => {
  it('is true when the target lies inside the leading download prefix', () => {
    expect(shouldRebuildFromPrefix(
      40_000,
      [
        { start: 0, end: 128_000 },
        { start: 512_000, end: 640_000 },
      ],
    )).toBe(true)
  })

  it('is false when the target is outside the leading prefix', () => {
    expect(shouldRebuildFromPrefix(
      200_000,
      [
        { start: 0, end: 128_000 },
        { start: 512_000, end: 640_000 },
      ],
    )).toBe(false)
  })

  it('is false when there is no leading prefix', () => {
    expect(shouldRebuildFromPrefix(
      10_000,
      [{ start: 512_000, end: 640_000 }],
    )).toBe(false)
  })
})

describe('nextAppendWindow', () => {
  it('returns the next inclusive chunk within the prefix', () => {
    expect(nextAppendWindow({
      nextAppendOffset: 0,
      prefixEnd: 300_000,
      total: 1_000_000,
      maxChunk: 128_000,
    })).toEqual({ start: 0, end: 127_999 })
  })

  it('advances from the current append offset', () => {
    expect(nextAppendWindow({
      nextAppendOffset: 128_000,
      prefixEnd: 200_000,
      total: 1_000_000,
      maxChunk: 128_000,
    })).toEqual({ start: 128_000, end: 199_999 })
  })

  it('returns null when caught up with the prefix or file', () => {
    expect(nextAppendWindow({
      nextAppendOffset: 200_000,
      prefixEnd: 200_000,
      total: 1_000_000,
      maxChunk: 128_000,
    })).toBeNull()

    expect(nextAppendWindow({
      nextAppendOffset: 1_000_000,
      prefixEnd: 1_000_000,
      total: 1_000_000,
      maxChunk: 128_000,
    })).toBeNull()
  })
})

describe('shouldEndOfStream', () => {
  it('is true only when download is complete and append caught up', () => {
    expect(shouldEndOfStream(1000, 1000, true)).toBe(true)
    expect(shouldEndOfStream(999, 1000, true)).toBe(false)
    expect(shouldEndOfStream(1000, 1000, false)).toBe(false)
  })
})

describe('appendedEndTime', () => {
  it('maps appended bytes onto duration proportionally', () => {
    expect(appendedEndTime(250, 1000, 200)).toBe(50)
    expect(appendedEndTime(0, 1000, 200)).toBe(0)
  })
})
