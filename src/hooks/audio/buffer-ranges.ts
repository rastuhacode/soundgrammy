import type { DownloadProgress } from '@/lib/api'

export interface TimeRange {
  start: number
  end: number
}

export function toCachedRanges(
  downloadProgress: DownloadProgress | null,
  duration: number,
): TimeRange[] {
  if (!(duration > 0 && downloadProgress?.total)) return []
  return downloadProgress.ranges.map(range => ({
    start: (range.start / downloadProgress.total) * duration,
    end: (range.end / downloadProgress.total) * duration,
  }))
}

export function timeIsInRanges(time: number, ranges: TimeRange[]): boolean {
  return ranges.some(range => time >= range.start && time < range.end)
}

/**
 * How far outside a buffered island we still treat a seek as landed.
 * Byte↔time mapping and decode delay often place the target slightly before
 * the first presentable frame after a discontinuous MSE append.
 */
export const SEEK_SNAP_TOLERANCE_SEC = 1.5

/**
 * Snap `time` into a nearby buffered range, or `null` if none are close.
 */
export function snapTimeToRanges(
  time: number,
  ranges: TimeRange[],
  tolerance = SEEK_SNAP_TOLERANCE_SEC,
): number | null {
  let best: { snapped: number, distance: number } | null = null

  for (const range of ranges) {
    if (!(range.end > range.start)) continue
    const insideEnd = Math.max(range.start, range.end - 0.05)

    let snapped: number
    let distance: number
    if (time < range.start) {
      snapped = range.start
      distance = range.start - time
    }
    else if (time >= range.end) {
      snapped = insideEnd
      distance = time - range.end
    }
    else {
      snapped = Math.min(time, insideEnd)
      distance = 0
    }

    if (distance <= tolerance && (!best || distance < best.distance)) {
      best = { snapped, distance }
    }
  }

  return best?.snapped ?? null
}

/**
 * Wider than {@link SEEK_SNAP_TOLERANCE_SEC} for post-discontinuity byte↔time
 * skew (VBR). Still bounded so a stale later island cannot absorb an earlier
 * scrub target while HTMLMediaElement.buffered lags SourceBuffer.remove.
 */
export const SEEK_LAND_TOLERANCE_SEC = 10

/**
 * After a discontinuous MSE seek, land on a nearby island even when
 * proportional byte↔time mapping misses by more than the normal snap window.
 */
export function landTimeOnRanges(
  time: number,
  ranges: TimeRange[],
): number | null {
  return snapTimeToRanges(time, ranges, SEEK_LAND_TOLERANCE_SEC)
}

/**
 * Buffer chrome for the progress bar.
 *
 * Prefer decoder `buffered` ranges (honest under MSE, including seek islands).
 * Fall back to append-tip / leading download prefix.
 */
export function computeBufferedRanges(options: {
  mediaRanges?: TimeRange[]
  playableEnd: number
  duration: number
  currentTime?: number
  /** Fallback before any bytes are appended (leading download prefix only). */
  cachedRanges?: TimeRange[]
}): TimeRange[] {
  const {
    mediaRanges = [],
    playableEnd,
    duration,
    currentTime = 0,
    cachedRanges = [],
  } = options
  if (!(duration > 0)) return []

  if (mediaRanges.length > 0) {
    // Single coherent range around the playhead / active island.
    const active = mediaRanges.find(
      range => currentTime >= range.start && currentTime < range.end,
    )
    if (active) return [active]
    // Pending seek into a new island — show the newest / last range.
    return [mediaRanges[mediaRanges.length - 1]!]
  }

  if (playableEnd > 0) {
    return [{ start: 0, end: Math.min(duration, playableEnd) }]
  }

  const leading = cachedRanges.find(range => range.start === 0)
  return leading ? [leading] : []
}
