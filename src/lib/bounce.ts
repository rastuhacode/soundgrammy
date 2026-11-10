export const BOUNCE_SETTINGS_KEY = 'soundgrammy-fullscreen-bounce-v1'

export interface BounceSettings {
  enabled: boolean
}

export const DEFAULT_BOUNCE_SETTINGS: BounceSettings = { enabled: false }

export interface BounceProfile {
  algorithmVersion: number
  frameMs: number
  durationMs: number
  loudness: Uint8Array
  onset: Uint8Array
  bass: Uint8Array
  mid: Uint8Array
  treble: Uint8Array
}

export function parseBounceSettings(raw: string | null): BounceSettings {
  if (!raw) return { ...DEFAULT_BOUNCE_SETTINGS }
  try {
    const value = JSON.parse(raw) as Partial<BounceSettings> | null
    // Keep the existing enabled preference; retire legacy manual tuning.
    return { enabled: typeof value?.enabled === 'boolean' ? value.enabled : DEFAULT_BOUNCE_SETTINGS.enabled }
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
  return Uint8Array.from(decoded, character => character.charCodeAt(0))
}

export function sampleProfileLane(lane: Uint8Array, timeSeconds: number, frameMs: number): number {
  if (lane.length === 0 || frameMs <= 0 || !Number.isFinite(timeSeconds)) return 0
  const position = Math.max(0, timeSeconds * 1000 / frameMs)
  if (position >= lane.length) return 0
  const left = Math.floor(position)
  const right = Math.min(lane.length - 1, left + 1)
  const mix = position - left
  return ((lane[left] ?? 0) * (1 - mix) + (lane[right] ?? 0) * mix) / 255
}

/** Cached on the profile, so frame rendering never scans a window of samples. */
export function onsetDensity(onset: Uint8Array, frameMs: number): Uint8Array {
  const radius = Math.max(1, Math.round(1000 / frameMs))
  const prefix = new Float64Array(onset.length + 1)
  for (let index = 0; index < onset.length; index++) {
    prefix[index + 1] = prefix[index] + (onset[index] > 48 ? 1 : 0)
  }
  return Uint8Array.from(onset, (_, index) => {
    const start = Math.max(0, index - radius)
    const end = Math.min(onset.length, index + radius + 1)
    return Math.round(255 * (prefix[end] - prefix[start]) / (end - start))
  })
}

/** Short positive changes within each frequency band, normalized independently.
 * A minimum contrast keeps sustained sound and quantization noise from pulsing.
 */
export function bandTransients(lane: Uint8Array, frameMs: number): Uint8Array {
  const lookback = Math.max(1, Math.round(75 / frameMs))
  const changes = Float32Array.from(lane, (value, index) => (
    Math.max(0, (value - lane[Math.max(0, index - lookback)]) / 255 - 0.025)
  ))
  const positive = [...changes].filter(value => value > 0).sort((a, b) => a - b)
  const reference = Math.max(0.18, positive[Math.floor((positive.length - 1) * 0.9)] ?? 0)
  return Uint8Array.from(changes, (change, index) => (
    Math.round(255 * Math.min(1, change / reference) * Math.min(1, lane[index] / 64))
  ))
}

function bytePercentile(values: Uint8Array, fraction: number): number {
  return values.slice().sort()[Math.floor((values.length - 1) * fraction)] ?? 0
}

export function adaptBounceProfile(profile: BounceProfile, transients: Uint8Array[]) {
  const peak = profile.loudness.reduce((max, value) => Math.max(max, value), 1)
  // An isolated maximum should not hold back the loud passages around it.
  const reference = Math.min(peak * 0.94, Math.max(peak * 0.85, bytePercentile(profile.loudness, 0.95)))
  const loudFrames = Array.from(profile.loudness.keys()).filter(index => profile.loudness[index] >= reference * 0.9)
  const upperBoost = (band: Uint8Array, attacks: Uint8Array) => {
    const level = bytePercentile(Uint8Array.from(loudFrames, index => band[index]), 0.5) / 255
    const attack = bytePercentile(Uint8Array.from(loudFrames, index => attacks[index]), 0.9) / 255
    // Recover smaller changes in loud, sustained bands while keeping quiet
    // bands and tracks with already clear transients on their original curve.
    const sustained = Math.min(1, Math.max(0, (level - 0.72) / 0.10))
    const headroom = Math.min(0.75, Math.max(0, 0.30 / Math.max(0.16, attack) - 1))
    return sustained * headroom
  }
  return {
    loudnessReference: reference / 255,
    paletteAccentBoosts: [0, upperBoost(profile.mid, transients[1]), upperBoost(profile.treble, transients[2])],
  }
}

export function bassMotionTiming(density: number): { attackMs: number, releaseMs: number } {
  const mix = Math.min(1, Math.max(0, density))
  return { attackMs: 30 + 10 * mix, releaseMs: 205 - 60 * mix }
}

export function bassMotionTarget(volumeGain: number, bassAttack: number, bassLevel: number, density: number): number {
  const clamp = (value: number) => Math.min(1, Math.max(0, value))
  const attack = clamp(bassAttack)
  // Bring out smaller hits over sustained bass without creating motion from a steady note.
  const accent = attack + 0.45 * clamp(bassLevel) * attack * (1 - attack)
  const gain = clamp(volumeGain)
  // Dense passages still have shorter motion, but their loudest hits keep full gain.
  return accent * gain / (1 + 0.65 * clamp(density) * (1 - gain))
}

export function followEnergy(current: number, target: number, elapsedMs: number, attackMs: number, releaseMs: number): number {
  const timing = target > current ? attackMs : releaseMs
  return current + (target - current) * (1 - Math.exp(-Math.max(0, elapsedMs) / timing))
}
