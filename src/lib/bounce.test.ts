import { describe, expect, it } from 'vitest'
import {
  DEFAULT_BOUNCE_SETTINGS,
  bounceEnergy,
  decodeProfileLane,
  motionTiming,
  parseBounceSettings,
  sampleProfileLane,
} from './bounce'

describe('bounce settings', () => {
  it('loads defaults and clamps compatible stored values', () => {
    expect(parseBounceSettings(null)).toEqual(DEFAULT_BOUNCE_SETTINGS)
    expect(parseBounceSettings(JSON.stringify({
      enabled: false,
      strength: 140,
      balance: -20,
      smoothness: 42.6,
    }))).toEqual({
      enabled: false,
      strength: 100,
      balance: 0,
      smoothness: 43,
    })
  })

  it('recovers from invalid storage', () => {
    expect(parseBounceSettings('{broken')).toEqual(DEFAULT_BOUNCE_SETTINGS)
    expect(parseBounceSettings(JSON.stringify({ strength: 'max' }))).toEqual(
      DEFAULT_BOUNCE_SETTINGS,
    )
  })
})

describe('bounce profile', () => {
  it('decodes and interpolates compact lanes', () => {
    const encoded = btoa(String.fromCharCode(0, 128, 255))
    const lane = decodeProfileLane(encoded)
    expect([...lane]).toEqual([0, 128, 255])
    expect(sampleProfileLane(lane, 0.025, 50)).toBeCloseTo(64 / 255, 2)
    expect(sampleProfileLane(lane, 99, 50)).toBe(1)
  })

  it('keeps energy bounded and preserves macro dynamics without onsets', () => {
    expect(bounceEnergy(1, 0, 45)).toBeCloseTo(0.7075)
    expect(bounceEnergy(0.2, 1, 45)).toBeGreaterThan(0.3)
    expect(bounceEnergy(20, 20, 50)).toBe(1)
    expect(bounceEnergy(-1, -1, 50)).toBe(0)
  })

  it('maps smoothness to deterministic attack and release ranges', () => {
    expect(motionTiming(0)).toEqual({ attackMs: 25, releaseMs: 140 })
    expect(motionTiming(100)).toEqual({ attackMs: 110, releaseMs: 420 })
  })
})
