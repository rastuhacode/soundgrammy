import { describe, expect, it } from 'vitest'
import {
  consumeVolumeWheelDelta,
  normalizeVolume,
  parseStoredVolume,
  VOLUME_DEFAULT,
} from './volume'

describe('normalizeVolume', () => {
  it('rounds to whole percentages and clamps to the supported range', () => {
    expect(normalizeVolume(42.4)).toBe(42)
    expect(normalizeVolume(42.5)).toBe(43)
    expect(normalizeVolume(-5)).toBe(0)
    expect(normalizeVolume(105)).toBe(100)
  })
})

describe('parseStoredVolume', () => {
  it('preserves compatible stored values while rounding old decimals', () => {
    expect(parseStoredVolume('67')).toBe(67)
    expect(parseStoredVolume('33.7')).toBe(34)
  })

  it('uses the safe default for missing or invalid values', () => {
    expect(parseStoredVolume(undefined)).toBe(VOLUME_DEFAULT)
    expect(parseStoredVolume('"loud"')).toBe(VOLUME_DEFAULT)
    expect(parseStoredVolume('-1')).toBe(VOLUME_DEFAULT)
    expect(parseStoredVolume('101')).toBe(VOLUME_DEFAULT)
  })
})

describe('consumeVolumeWheelDelta', () => {
  it('maps mouse-wheel detents to one-percent changes', () => {
    expect(consumeVolumeWheelDelta(-100, 0, 0)).toEqual({
      step: 1,
      remainder: 0,
    })
    expect(consumeVolumeWheelDelta(100, 0, 0)).toEqual({
      step: -1,
      remainder: 0,
    })
    expect(consumeVolumeWheelDelta(-3, 1, 0)).toEqual({
      step: 1,
      remainder: 0,
    })
  })

  it('accumulates small trackpad deltas before emitting a step', () => {
    const first = consumeVolumeWheelDelta(15, 0, 0)
    const second = consumeVolumeWheelDelta(15, 0, first.remainder)
    const third = consumeVolumeWheelDelta(10, 0, second.remainder)

    expect(first).toEqual({ step: 0, remainder: 15 })
    expect(second).toEqual({ step: 0, remainder: 30 })
    expect(third).toEqual({ step: -1, remainder: 0 })
  })

  it('drops accumulated momentum when the gesture changes direction', () => {
    expect(consumeVolumeWheelDelta(-20, 0, 20)).toEqual({
      step: 0,
      remainder: -20,
    })
  })
})
