import { describe, expect, it } from 'vitest'
import {
  DEFAULT_BOUNCE_SETTINGS,
  adaptBounceProfile,
  bandTransients,
  bassMotionTarget,
  bassMotionTiming,
  decodeProfileLane,
  followEnergy,
  onsetDensity,
  parseBounceSettings,
  sampleProfileLane,
} from './bounce'

describe('bounce settings', () => {
  it('preserves the old enabled preference and retires manual tuning', () => {
    expect(parseBounceSettings(null)).toEqual(DEFAULT_BOUNCE_SETTINGS)
    expect(parseBounceSettings(JSON.stringify({ enabled: true, strength: 140, balance: -20, smoothness: 42.6 }))).toEqual({ enabled: true })
  })

  it('recovers from invalid storage', () => {
    for (const value of ['{broken', 'null', 'false', '{"enabled":"yes"}']) {
      expect(parseBounceSettings(value)).toEqual(DEFAULT_BOUNCE_SETTINGS)
    }
  })
})

describe('adaptive motion', () => {
  it('keeps an isolated peak from suppressing loud sections and gives sustained bands more contrast', () => {
    const lane = (value: number) => new Uint8Array(100).fill(value)
    const loudness = lane(225)
    loudness[0] = 255
    const profile = {
      algorithmVersion: 2,
      frameMs: 25,
      durationMs: 2500,
      loudness,
      onset: lane(0),
      bass: lane(200),
      mid: lane(210),
      treble: lane(30),
    }
    const attacks = [lane(120), lane(45), lane(45)]
    const adapted = adaptBounceProfile(profile, attacks)

    expect(adapted.loudnessReference).toBeCloseTo(225 / 255)
    expect(adapted.paletteAccentBoosts[1]).toBeGreaterThan(0.4)
    expect(adapted.paletteAccentBoosts[2]).toBe(0)

    const reference = adaptBounceProfile({ ...profile, loudness: lane(230) }, [lane(120), lane(120), lane(120)])
    expect(reference.loudnessReference).toBeCloseTo(230 * 0.94 / 255)
    expect(reference.paletteAccentBoosts).toEqual([0, 0, 0])
  })

  it('ignores steady level and slow swells while preserving repeated attacks', () => {
    expect([...bandTransients(new Uint8Array(100).fill(230), 25)].every(value => value === 0)).toBe(true)
    const swell = Uint8Array.from({ length: 100 }, (_, index) => 100 + index)
    expect([...bandTransients(swell, 25)].every(value => value === 0)).toBe(true)
    const hits = new Uint8Array(100).fill(80)
    hits.fill(220, 20, 22)
    hits.fill(220, 60, 62)
    const accents = bandTransients(hits, 25)
    expect(accents[20]).toBeGreaterThan(200)
    expect(accents[60]).toBeGreaterThan(200)
    expect(accents[25]).toBe(0)
    expect(accents[65]).toBe(0)
  })

  it('decodes and interpolates compact lanes without holding energy past EOF', () => {
    const lane = decodeProfileLane(btoa(String.fromCharCode(0, 128, 255)))
    expect([...lane]).toEqual([0, 128, 255])
    expect(sampleProfileLane(lane, 0.025, 50)).toBeCloseTo(64 / 255, 2)
    expect(sampleProfileLane(lane, 99, 50)).toBe(0)
    expect(sampleProfileLane(lane, Number.NaN, 50)).toBe(0)
  })

  it('brings out smaller attacks over strong bass without moving on sustained bass', () => {
    const isolated = bassMotionTarget(0.8, 1, 0, 0)
    expect(bassMotionTarget(0.8, 1, 0, 1)).toBeLessThan(isolated)
    expect(bassMotionTarget(1, 0.25, 0.8, 0)).toBeGreaterThan(bassMotionTarget(1, 0.25, 0.2, 0))
    expect(bassMotionTarget(1, 1, 1, 1)).toBe(1)
    expect(bassMotionTarget(0.05, 1, 0, 0)).toBeLessThan(isolated)
    expect(bassMotionTarget(1, 0, 1, 0)).toBe(0)
  })

  it('adapts timing to measured onset density', () => {
    const sparse = onsetDensity(Uint8Array.from([0, 0, 255, 0, 0]), 25)
    const dense = onsetDensity(new Uint8Array(100).fill(255), 25)
    expect(sparse[2]).toBeLessThan(dense[50])
    expect(bassMotionTiming(sparse[2] / 255).releaseMs).toBeGreaterThan(bassMotionTiming(dense[50] / 255).releaseMs)
  })

  it('has frame-rate independent attack/release and settles toward silence', () => {
    const full = followEnergy(0, 1, 32, 30, 230)
    const half = followEnergy(followEnergy(0, 1, 16, 30, 230), 1, 16, 30, 230)
    expect(full).toBeCloseTo(half)
    expect(followEnergy(1, 0, 1000, 30, 230)).toBeLessThan(0.02)
    expect(followEnergy(0, 1, 0, 30, 230)).toBe(0)
  })
})
