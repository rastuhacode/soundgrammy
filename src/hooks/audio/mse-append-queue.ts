/** Half-open byte range matching download:progress ledger. */
export interface ByteRange {
  start: number
  end: number
}

/** Inclusive byte window for `read_stream_range`. */
export interface InclusiveByteWindow {
  start: number
  end: number
}

/** End (exclusive) of the contiguous prefix starting at byte 0. */
export function leadingPrefixEnd(ranges: ByteRange[]): number {
  const leading = ranges.find(range => range.start === 0)
  return leading?.end ?? 0
}

/**
 * Prefer sequential rebuild from byte 0 when the seek target still lies inside
 * the downloaded leading prefix. Avoids backward `timestampOffset` jumps on
 * `audio/mpeg` (sequence + generateTimestamps), which deadlock on WebKit.
 */
export function shouldRebuildFromPrefix(
  targetByte: number,
  ranges: ByteRange[],
): boolean {
  if (!(targetByte >= 0)) return false
  const prefixEnd = leadingPrefixEnd(ranges)
  return prefixEnd > 0 && targetByte < prefixEnd
}

/**
 * Next inclusive read window to append, or `null` when the SourceBuffer is
 * caught up with the downloaded prefix (or the full file).
 */
export function nextAppendWindow(options: {
  nextAppendOffset: number
  prefixEnd: number
  total: number
  maxChunk: number
}): InclusiveByteWindow | null {
  const { nextAppendOffset, prefixEnd, total, maxChunk } = options
  if (!(total > 0) || !(maxChunk > 0)) return null
  if (nextAppendOffset >= total || nextAppendOffset >= prefixEnd) return null

  const endExclusive = Math.min(
    prefixEnd,
    total,
    nextAppendOffset + maxChunk,
  )
  if (endExclusive <= nextAppendOffset) return null
  return { start: nextAppendOffset, end: endExclusive - 1 }
}

export function shouldEndOfStream(
  nextAppendOffset: number,
  total: number,
  complete: boolean,
): boolean {
  return complete && total > 0 && nextAppendOffset >= total
}

/** Estimated media time covered by appended bytes (proportional mapping). */
export function appendedEndTime(
  nextAppendOffset: number,
  total: number,
  duration: number,
): number {
  if (!(duration > 0 && total > 0)) return 0
  return Math.min(duration, (nextAppendOffset / total) * duration)
}
