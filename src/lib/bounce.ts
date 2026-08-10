export const BOUNCE_SETTINGS_KEY = 'soundgrammy-fullscreen-bounce-v1'

export interface BounceSettings {
  enabled: boolean
  strength: number
  balance: number
  smoothness: number
}

export const DEFAULT_BOUNCE_SETTINGS: BounceSettings = {
  enabled: false,
  strength: 55,
  balance: 45,
  smoothness: 60,
}

export interface BounceProfile {
  algorithmVersion: number
  frameMs: number
  durationMs: number
  loudness: Uint8Array
  onset: Uint8Array
}

function clampPercent(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.round(Math.min(100, Math.max(0, value)))
    : fallback
}

export function parseBounceSettings(raw: string | null): BounceSettings {
  if (!raw) return { ...DEFAULT_BOUNCE_SETTINGS }
  try {
    const value = JSON.parse(raw) as Partial<BounceSettings> | null
    if (!value || typeof value !== 'object') return { ...DEFAULT_BOUNCE_SETTINGS }
    return {
      enabled: typeof value.enabled === 'boolean'
        ? value.enabled
        : DEFAULT_BOUNCE_SETTINGS.enabled,
      strength: clampPercent(value.strength, DEFAULT_BOUNCE_SETTINGS.strength),
      balance: clampPercent(value.balance, DEFAULT_BOUNCE_SETTINGS.balance),
      smoothness: clampPercent(value.smoothness, DEFAULT_BOUNCE_SETTINGS.smoothness),
    }
  }
  catch {
    return { ...DEFAULT_BOUNCE_SETTINGS }
  }
}

export function loadBounceSettings(): BounceSettings {
  if (typeof window === 'undefined') return { ...DEFAULT_BOUNCE_SETTINGS }
  return parseBounceSettings(window.localStorage.getItem(BOUNCE_SETTINGS_KEY))
}

export function saveBounceSettings(settings: BounceSettings) {
  if (typeof window === 'undefined') return
  window.localStorage.setItem(BOUNCE_SETTINGS_KEY, JSON.stringify(settings))
}

export function decodeProfileLane(encoded: string): Uint8Array {
  const decoded = globalThis.atob(encoded)
  const values = new Uint8Array(decoded.length)
  for (let index = 0; index < decoded.length; index += 1) {
    values[index] = decoded.charCodeAt(index)
  }
  return values
}

export function sampleProfileLane(
  lane: Uint8Array,
  timeSeconds: number,
  frameMs: number,
): number {
  if (lane.length === 0 || frameMs <= 0 || !Number.isFinite(timeSeconds)) return 0
  const position = Math.max(0, timeSeconds * 1000 / frameMs)
  const left = Math.min(lane.length - 1, Math.floor(position))
  const right = Math.min(lane.length - 1, left + 1)
  const mix = position - Math.floor(position)
  return ((lane[left] ?? 0) * (1 - mix) + (lane[right] ?? 0) * mix) / 255
}

export function bounceEnergy(loudness: number, onset: number, balance: number): number {
  const dynamics = Math.min(1, Math.max(0, loudness))
  const beats = Math.min(1, Math.max(0, onset))
  const mix = Math.min(1, Math.max(0, balance / 100))
  const accent = beats * (0.35 + 0.65 * dynamics)
  return Math.min(1, Math.max(
    0,
    dynamics * (1 - 0.65 * mix) + accent * (0.35 + 0.65 * mix),
  ))
}

export function motionTiming(smoothness: number): { attackMs: number, releaseMs: number } {
  const mix = Math.min(1, Math.max(0, smoothness / 100))
  return {
    attackMs: 25 + (110 - 25) * mix,
    releaseMs: 140 + (420 - 140) * mix,
  }
}
