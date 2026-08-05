export const VOLUME_MIN = 0
export const VOLUME_MAX = 100
export const VOLUME_DEFAULT = 25

const TRACKPAD_WHEEL_THRESHOLD = 40

export function normalizeVolume(value: number): number {
  if (!Number.isFinite(value)) return VOLUME_MIN

  return Math.min(VOLUME_MAX, Math.max(VOLUME_MIN, Math.round(value)))
}

export function parseStoredVolume(stored: string | undefined): number {
  if (stored === undefined) return VOLUME_DEFAULT

  let value: unknown
  try {
    value = JSON.parse(stored)
  }
  catch {
    value = Number(stored)
  }

  if (
    typeof value !== 'number'
    || !Number.isFinite(value)
    || value < VOLUME_MIN
    || value > VOLUME_MAX
  ) {
    return VOLUME_DEFAULT
  }

  return normalizeVolume(value)
}

export interface VolumeWheelResult {
  /** Signed volume change: scroll up is +1, scroll down is -1. */
  step: -1 | 0 | 1
  /** Unconsumed pixel delta for high-resolution trackpad gestures. */
  remainder: number
}

/**
 * Converts wheel input into deliberate one-percent steps. Mouse-wheel detents
 * arrive as line/page deltas or large pixel deltas; small pixel deltas are
 * accumulated for trackpads so a single gesture cannot race through volume.
 */
export function consumeVolumeWheelDelta(
  deltaY: number,
  deltaMode: number,
  remainder: number,
): VolumeWheelResult {
  if (!Number.isFinite(deltaY) || deltaY === 0) {
    return { step: 0, remainder }
  }

  const direction = Math.sign(deltaY)

  if (deltaMode !== 0 || Math.abs(deltaY) >= TRACKPAD_WHEEL_THRESHOLD) {
    return { step: direction > 0 ? -1 : 1, remainder: 0 }
  }

  const sameDirection = remainder === 0 || Math.sign(remainder) === direction
  const nextRemainder = (sameDirection ? remainder : 0) + deltaY
  if (Math.abs(nextRemainder) < TRACKPAD_WHEEL_THRESHOLD) {
    return { step: 0, remainder: nextRemainder }
  }

  return {
    step: direction > 0 ? -1 : 1,
    remainder: nextRemainder - direction * TRACKPAD_WHEEL_THRESHOLD,
  }
}
