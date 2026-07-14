import type { DownloadProgress } from '@/lib/api'

export interface TimeRange {
  start: number
  end: number
}

export interface BufferAnchor {
  time: number
  received: number
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

export function computeBufferedRanges(options: {
  cachedRanges: TimeRange[]
  bufferAnchor: BufferAnchor | null
  downloadProgress: DownloadProgress | null
  duration: number
  currentTime: number
}): TimeRange[] {
  const {
    cachedRanges,
    bufferAnchor,
    downloadProgress,
    duration,
    currentTime,
  } = options

  const leadingRange = cachedRanges.find(range => range.start === 0)
  const leadingIncludesAnchor = bufferAnchor !== null
    && leadingRange
    && bufferAnchor.time < leadingRange.end

  if (bufferAnchor !== null && !leadingIncludesAnchor) {
    const receivedSinceSeek = Math.max(
      0,
      (downloadProgress?.received ?? bufferAnchor.received)
      - bufferAnchor.received,
    )
    if (receivedSinceSeek > 0 && downloadProgress?.total) {
      const bufferedDuration
        = (receivedSinceSeek / downloadProgress.total) * duration
      return [{
        start: bufferAnchor.time,
        end: Math.min(duration, bufferAnchor.time + bufferedDuration),
      }]
    }
    return []
  }

  const activeRange = cachedRanges.find(
    range => currentTime >= range.start && currentTime < range.end,
  )
  const displayedRange = activeRange ?? leadingRange ?? cachedRanges[0]
  return displayedRange ? [displayedRange] : []
}
