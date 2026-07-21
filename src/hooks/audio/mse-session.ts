import { api, type DownloadProgress } from '@/lib/api'
import {
  leadingPrefixEnd,
  nextAppendWindow,
  shouldEndOfStream,
  shouldRebuildFromPrefix,
  type ByteRange,
} from './mse-append-queue'
import { resolveFrameSyncOffset, resolveMpegPayloadStart, parseMp3FrameAt, completeMpegFrameByteLength } from './mp3-frame-sync'

/** Matches Rust streaming::CHUNK_SIZE — keep append IPC payloads bounded. */
export const MSE_APPEND_CHUNK = 128 * 1024

/** Bytes before the seek target to search for a frame sync. */
const SEEK_PROBE_BACK = 4 * 1024

export function isMseTypeSupported(mimeType: string): boolean {
  return resolveMseMimeType(mimeType) !== null
}

/** Pick a SourceBuffer MIME WebKit/Chrome will actually accept. */
export function resolveMseMimeType(mimeType: string): string | null {
  if (
    typeof MediaSource === 'undefined'
    || typeof MediaSource.isTypeSupported !== 'function'
  ) {
    return null
  }
  for (const candidate of mseMimeCandidates(mimeType)) {
    if (MediaSource.isTypeSupported(candidate)) return candidate
  }
  return null
}

function mseMimeCandidates(mimeType: string): string[] {
  const base = mimeType.split(';')[0]?.trim().toLowerCase() || 'audio/mpeg'
  switch (base) {
    case 'audio/webm':
    case 'video/webm':
      return [
        'audio/webm; codecs="opus"',
        'audio/webm; codecs="vorbis"',
        'audio/webm',
      ]
    case 'audio/ogg':
    case 'audio/opus':
      return [
        'audio/ogg; codecs="opus"',
        'audio/ogg; codecs="vorbis"',
        'audio/ogg',
      ]
    case 'audio/mp4':
    case 'audio/m4a':
    case 'audio/x-m4a':
      return [
        'audio/mp4; codecs="mp4a.40.2"',
        'audio/mp4',
      ]
    case 'audio/mpeg':
    case 'audio/mp3':
      return ['audio/mpeg']
    default:
      return [mimeType, base]
  }
}

function isMpegMseMime(mimeType: string): boolean {
  const base = mimeType.split(';')[0]?.trim().toLowerCase() || ''
  return base === 'audio/mpeg' || base === 'audio/mp3'
}

/** Raw MPEG has no timestamps; WebM/MP4 do — use segments for the latter. */
function preferredBufferMode(mimeType: string): 'sequence' | 'segments' {
  return isMpegMseMime(mimeType) ? 'sequence' : 'segments'
}

export interface MseSessionCallbacks {
  onAppendedOffset?: (offset: number) => void
  onBufferedChanged?: () => void
  onError?: () => void
}

export interface AttachMseSessionOptions extends MseSessionCallbacks {
  audio: HTMLAudioElement
  trackId: number
  mimeType: string
  total: number
  duration: number
}

export interface MseSession {
  readonly objectUrl: string
  notifyProgress: (progress: DownloadProgress) => void
  getAppendedOffset: () => number
  /**
   * Snap `time` onto SourceBuffer buffered ranges (not HTMLMediaElement).
   * Returns null when the target is outside the snap window.
   */
  snapToBufferedTime: (time: number) => number | null
  /**
   * Land on the active SourceBuffer island after a discontinuous append.
   * Safer than HTMLMediaElement.buffered, which can keep stale ranges on WebKit.
   */
  landToBufferedTime: (time: number) => number | null
  /**
   * Discontinuous seek: either rebuild the leading prefix from byte 0, or
   * recreate the SourceBuffer and append a mid-file island.
   */
  seekToTime: (time: number) => Promise<void>
  dispose: () => void
}

function waitForSourceBufferIdle(sourceBuffer: SourceBuffer): Promise<void> {
  if (!sourceBuffer.updating) return Promise.resolve()
  return new Promise((resolve) => {
    const onEnd = () => {
      sourceBuffer.removeEventListener('updateend', onEnd)
      sourceBuffer.removeEventListener('error', onEnd)
      resolve()
    }
    sourceBuffer.addEventListener('updateend', onEnd)
    sourceBuffer.addEventListener('error', onEnd)
  })
}

/** WebKit throws InvalidStateError reading `.buffered` during some MSE updates. */
function readBufferedRanges(
  buffer: SourceBuffer,
): Array<{ start: number, end: number }> {
  try {
    const ranges: Array<{ start: number, end: number }> = []
    for (let i = 0; i < buffer.buffered.length; i++) {
      ranges.push({
        start: buffer.buffered.start(i),
        end: buffer.buffered.end(i),
      })
    }
    return ranges
  }
  catch {
    return []
  }
}

function bufferCoversTime(buffer: SourceBuffer, time: number): boolean {
  for (const range of readBufferedRanges(buffer)) {
    if (time >= range.start && time < range.end) return true
  }
  return false
}

function snapTimeIntoBuffer(
  buffer: SourceBuffer,
  time: number,
  tolerance = 1.5,
): number | null {
  const ranges = readBufferedRanges(buffer)
  if (ranges.length === 0) return null
  let best: { snapped: number, distance: number } | null = null
  for (const range of ranges) {
    const start = range.start
    const end = range.end
    if (!(end > start)) continue
    const insideEnd = Math.max(start, end - 0.05)
    let snapped: number
    let distance: number
    if (time < start) {
      snapped = start
      distance = start - time
    }
    else if (time >= end) {
      snapped = insideEnd
      distance = time - end
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

/** After an island append, land on the only buffered range even if byte↔time is off. */
function landOnBufferedIsland(buffer: SourceBuffer, time: number): number | null {
  const near = snapTimeIntoBuffer(buffer, time, 1.5)
  if (near !== null) return near
  const ranges = readBufferedRanges(buffer)
  if (ranges.length === 0) return null
  // Proportional byte mapping can miss by more than 1.5s on VBR — still land.
  const start = ranges[0]!.start
  const end = ranges[ranges.length - 1]!.end
  const insideEnd = Math.max(start, end - 0.05)
  if (time < start) return start
  if (time >= end) return insideEnd
  return Math.min(time, insideEnd)
}

/**
 * Attaches `audio` to a MediaSource and appends downloaded bytes.
 * Supports sequential prefix growth and discontinuous island seeks.
 */
export function attachMseSession(options: AttachMseSessionOptions): MseSession {
  const {
    audio,
    trackId,
    mimeType,
    total,
    duration,
    onAppendedOffset,
    onBufferedChanged,
    onError,
  } = options

  let mediaSource = new MediaSource()
  let objectUrl = URL.createObjectURL(mediaSource)
  let sourceBuffer: SourceBuffer | null = null
  let nextAppendOffset = 0
  let disposed = false
  let appendInFlight = false
  let ended = false
  let latestRanges: ByteRange[] = []
  let latestComplete = false
  let pumpQueued = false
  let seekGeneration = 0
  let discontinuityPending = false
  // Skip ID3v2 (often multi‑MB album art) before the first MPEG frame.
  let mpegPayloadStart = 0
  let mpegStartResolved = !isMpegMseMime(mimeType)
  // Library metadata can be 0; adopt element duration once it is known.
  let resolvedDuration = duration > 0 ? duration : 0

  const resolveDuration = (): number => {
    if (resolvedDuration > 0) return resolvedDuration
    if (Number.isFinite(audio.duration) && audio.duration > 0) {
      resolvedDuration = audio.duration
      if (mediaSource.readyState === 'open') {
        try {
          mediaSource.duration = resolvedDuration
        }
        catch {
          // ignore if duration already set
        }
      }
    }
    return resolvedDuration
  }

  const ensureMpegPayloadStart = async (): Promise<boolean> => {
    if (mpegStartResolved || disposed || !(total > 10)) {
      mpegStartResolved = true
      return true
    }
    try {
      const headerEnd = Math.min(total - 1, 9)
      await api.ensureStreamRange(trackId, 0, headerEnd)
      if (disposed) return false
      const header = await api.readStreamRange(trackId, 0, headerEnd)
      if (disposed) return false
      const id3Start = resolveMpegPayloadStart(header, total)
      let start = Math.max(0, Math.min(total - 1, id3Start))

      // Probe around the ID3-derived offset: overshooting skips the intro
      // (sequence mode still stamps the first decoded frame as t=0).
      const probeBack = Math.min(start, 4 * 1024)
      const probeAhead = Math.min(total - 1, start + 8 * 1024)
      const probeFrom = start - probeBack
      await api.ensureStreamRange(trackId, probeFrom, probeAhead)
      if (disposed) return false
      const probe = await api.readStreamRange(trackId, probeFrom, probeAhead)
      if (disposed) return false

      const relAtId3 = start - probeFrom
      const frameAtId3 = parseMp3FrameAt(probe, relAtId3)

      // Match AVFoundation: start after Xing/Info so the first coded frame is
      // real audio (first audio frame typically has main_data_begin=0).
      if (frameAtId3) {
        const markerOff = relAtId3 + 36
        const marker = String.fromCharCode(
          probe[markerOff] ?? 0,
          probe[markerOff + 1] ?? 0,
          probe[markerOff + 2] ?? 0,
          probe[markerOff + 3] ?? 0,
        )
        if (marker === 'Xing' || marker === 'Info') {
          start = start + frameAtId3.size
        }
      }

      mpegPayloadStart = Math.max(0, Math.min(total - 1, start))
      mpegStartResolved = true
      if (nextAppendOffset < mpegPayloadStart) {
        // Advance the append cursor past the ID3 tag, but do not report
        // appendedBytes yet — that would paint fake buffer chrome before any
        // MPEG frames are decoded.
        nextAppendOffset = mpegPayloadStart
      }
      return true
    }
    catch {
      mpegStartResolved = true
      fail()
      return false
    }
  }

  const emitAppended = () => {
    onAppendedOffset?.(nextAppendOffset)
    onBufferedChanged?.()
  }

  const fail = () => {
    if (disposed) return
    onError?.()
  }

  const tryEndOfStream = () => {
    if (
      disposed
      || ended
      || discontinuityPending
      || !sourceBuffer
      || sourceBuffer.updating
      || mediaSource.readyState !== 'open'
    ) {
      return
    }
    if (!shouldEndOfStream(nextAppendOffset, total, latestComplete)) return

    // WebKit's endOfStream() clamps MediaSource.duration to SourceBuffer
    // buffered end. Doing that mid-track can seek currentTime onto the clamp
    // and fire `ended` / pause. Keep the source open until the playhead is
    // near the real end; timeupdate retries this.
    const dur = resolveDuration()
    const current = audio.currentTime
    if (Number.isFinite(current) && current > 0) {
      const ranges = readBufferedRanges(sourceBuffer)
      const bufferedEnd = ranges.length > 0 ? ranges[ranges.length - 1]!.end : 0
      const endAnchor = dur > 0
        ? (bufferedEnd > 0 ? Math.min(dur, bufferedEnd) : dur)
        : bufferedEnd
      if (endAnchor > 0 && current < endAnchor - 1.25) {
        return
      }
    }

    try {
      mediaSource.endOfStream()
      ended = true
    }
    catch {
      // InvalidStateError if a concurrent update started — retry on next
      // updateend / timeupdate. Never treat EOS failure as a playback error.
    }
  }

  const onAudioTimeUpdate = () => {
    if (!disposed) tryEndOfStream()
  }

  const onSourceBufferUpdateEnd = () => {
    if (disposed) return
    onBufferedChanged?.()
    tryEndOfStream()
    schedulePump()
  }

  const configureSourceBuffer = (buffer: SourceBuffer) => {
    try {
      buffer.mode = preferredBufferMode(mimeType)
    }
    catch {
      // ignore unsupported mode / mode already locked after first append
    }
    if (Number.isFinite(resolvedDuration) && resolvedDuration > 0) {
      try {
        mediaSource.duration = resolvedDuration
      }
      catch {
        // ignore if duration already set
      }
    }
    buffer.addEventListener('updateend', onSourceBufferUpdateEnd)
    buffer.addEventListener('error', fail)
  }

  /** Clears buffered media. Returns false if remove failed — callers must abort the seek. */
  const clearBuffered = async (buffer: SourceBuffer): Promise<boolean> => {
    await waitForSourceBufferIdle(buffer)
    if (disposed || mediaSource.readyState !== 'open') return false
    if (buffer.updating) {
      try {
        buffer.abort()
      }
      catch {
        // ignore
      }
      await waitForSourceBufferIdle(buffer)
    }
    if (disposed || mediaSource.readyState !== 'open') return false
    const existing = readBufferedRanges(buffer)
    if (existing.length === 0) return true
    const bufferedEnd = existing[existing.length - 1]!.end
    const removeEnd = Number.isFinite(mediaSource.duration) && mediaSource.duration > 0
      ? Math.max(mediaSource.duration, bufferedEnd)
      : bufferedEnd + 1
    try {
      buffer.remove(0, removeEnd)
    }
    catch {
      // Do not continue a discontinuous seek on an uncleared buffer — overlapping
      // MP3 timeline data breaks WebKit seeks.
      return false
    }
    await waitForSourceBufferIdle(buffer)
    return !disposed && mediaSource.readyState === 'open'
  }

  const setAudioTimeInBuffer = (buffer: SourceBuffer, time: number) => {
    const landed = landOnBufferedIsland(buffer, time)
    if (landed === null) return
    try {
      audio.currentTime = landed
    }
    catch {
      // seek will retry via canplay
    }
  }

  const appendChunk = async (
    buffer: SourceBuffer,
    start: number,
    end: number,
    generation: number,
  ): Promise<boolean> => {
    const bytes = await api.readStreamRange(trackId, start, end)
    if (
      disposed
      || generation !== seekGeneration
      || !sourceBuffer
      || sourceBuffer !== buffer
      || mediaSource.readyState !== 'open'
      || bytes.byteLength === 0
    ) {
      return false
    }
    await waitForSourceBufferIdle(buffer)
    if (
      disposed
      || generation !== seekGeneration
      || mediaSource.readyState !== 'open'
    ) {
      return false
    }

    let appendBytes = bytes
    let nextOffset = end + 1
    // Keep MPEG appends frame-aligned except on the final EOF window.
    if (isMpegMseMime(mimeType) && end < total - 1) {
      const keep = completeMpegFrameByteLength(bytes)
      if (keep <= 0) return false
      if (keep < bytes.byteLength) {
        appendBytes = bytes.subarray(0, keep)
        nextOffset = start + keep
      }
    }

    try {
      buffer.appendBuffer(new Uint8Array(appendBytes))
    }
    catch {
      return false
    }
    await waitForSourceBufferIdle(buffer)
    if (disposed || generation !== seekGeneration) return false
    nextAppendOffset = nextOffset
    emitAppended()
    return true
  }

  /**
   * Grow the existing leading buffer until `clampedTime` is covered.
   * WebM/MP4: do not clear+rebuild — WebKit closes MediaSource after
   * remove()+re-append of the init segment mid-session.
   */
  const growUntilTime = async (
    buffer: SourceBuffer,
    clampedTime: number,
    generation: number,
  ) => {
    ended = false
    const ranges = readBufferedRanges(buffer)
    let lastBufferedEnd = ranges.length > 0 ? ranges[ranges.length - 1]!.end : 0
    let stagnantAppends = 0

    while (
      !disposed
      && generation === seekGeneration
      && nextAppendOffset < total
      && !bufferCoversTime(buffer, clampedTime)
    ) {
      const end = Math.min(total - 1, nextAppendOffset + MSE_APPEND_CHUNK - 1)
      if (end < nextAppendOffset) break

      await api.ensureStreamRange(trackId, nextAppendOffset, end)
      if (disposed || generation !== seekGeneration) return

      const ok = await appendChunk(buffer, nextAppendOffset, end, generation)
      if (!ok) return

      const nextRanges = readBufferedRanges(buffer)
      const bufferedEnd = nextRanges.length > 0
        ? nextRanges[nextRanges.length - 1]!.end
        : 0
      if (bufferedEnd <= lastBufferedEnd + 0.01) {
        stagnantAppends += 1
        const limit = lastBufferedEnd > 0 ? 8 : 24
        if (stagnantAppends >= limit) break
      }
      else {
        stagnantAppends = 0
        lastBufferedEnd = bufferedEnd
      }
    }

    if (disposed || generation !== seekGeneration) return
    // Stagnant or incomplete growth: do not snap a far target onto the buffer tip
    // and pretend the seek landed.
    if (snapTimeIntoBuffer(buffer, clampedTime) === null) {
      fail()
      return
    }
    setAudioTimeInBuffer(buffer, clampedTime)
  }

  /**
   * Non-MP3: after a fatal WebKit MediaSource close, attach a fresh MSE pipeline
   * and rebuild from byte 0 until the seek time is covered.
   */
  const reattachAndRebuild = async (
    clampedTime: number,
    generation: number,
  ): Promise<void> => {
    const previousUrl = objectUrl
    mediaSource = new MediaSource()
    objectUrl = URL.createObjectURL(mediaSource)
    sourceBuffer = null
    nextAppendOffset = 0
    ended = false

    const opened = new Promise<boolean>((resolve) => {
      const onOpen = () => {
        mediaSource.removeEventListener('sourceopen', onOpen)
        resolve(true)
      }
      mediaSource.addEventListener('sourceopen', onOpen)
      mediaSource.addEventListener('error', () => resolve(false), { once: true })
    })
    audio.src = objectUrl
    try {
      URL.revokeObjectURL(previousUrl)
    }
    catch {
      // ignore
    }
    const ok = await opened
    if (!ok || disposed || generation !== seekGeneration) {
      fail()
      return
    }
    try {
      sourceBuffer = mediaSource.addSourceBuffer(mimeType)
      configureSourceBuffer(sourceBuffer)
    }
    catch {
      fail()
      return
    }
    await growUntilTime(sourceBuffer, clampedTime, generation)
  }

  /** Rebuild timeline from byte 0 until `clampedTime` is inside buffered. */
  const rebuildFromPrefix = async (
    buffer: SourceBuffer,
    clampedTime: number,
    generation: number,
  ) => {
    const prefixEnd = leadingPrefixEnd(latestRanges)
    await appendPrefixUntil(buffer, clampedTime, prefixEnd, generation)
  }

  const appendPrefixUntil = async (
    buffer: SourceBuffer,
    clampedTime: number,
    prefixEndLimit: number,
    generation: number,
  ) => {
    nextAppendOffset = mpegPayloadStart
    ended = false
    try {
      if (!buffer.updating) buffer.timestampOffset = 0
    }
    catch {
      // segments mode may reject offset changes; continue and append from start
    }
    emitAppended()

    let lastBufferedEnd = 0
    let stagnantAppends = 0

    while (
      !disposed
      && generation === seekGeneration
      && nextAppendOffset < prefixEndLimit
      && nextAppendOffset < total
      && !bufferCoversTime(buffer, clampedTime)
    ) {
      const end = Math.min(
        total - 1,
        prefixEndLimit - 1,
        nextAppendOffset + MSE_APPEND_CHUNK - 1,
      )
      if (end < nextAppendOffset) break

      await api.ensureStreamRange(trackId, nextAppendOffset, end)
      if (disposed || generation !== seekGeneration) return

      const ok = await appendChunk(buffer, nextAppendOffset, end, generation)
      if (!ok) return

      const ranges = readBufferedRanges(buffer)
      const bufferedEnd = ranges.length > 0 ? ranges[ranges.length - 1]!.end : 0
      if (bufferedEnd <= lastBufferedEnd + 0.01) {
        stagnantAppends += 1
        const limit = lastBufferedEnd > 0 ? 8 : 24
        if (stagnantAppends >= limit) break
      }
      else {
        stagnantAppends = 0
        lastBufferedEnd = bufferedEnd
      }
    }

    if (disposed || generation !== seekGeneration) return
    if (snapTimeIntoBuffer(buffer, clampedTime) === null) {
      fail()
      return
    }
    setAudioTimeInBuffer(buffer, clampedTime)
  }

  const appendIsland = async (
    buffer: SourceBuffer,
    clampedTime: number,
    targetByte: number,
    generation: number,
  ) => {
    const probeStart = Math.max(0, targetByte - SEEK_PROBE_BACK)
    const probeEnd = Math.min(total - 1, targetByte + MSE_APPEND_CHUNK - 1)

    await api.ensureStreamRange(trackId, probeStart, probeEnd)
    if (disposed || generation !== seekGeneration) return

    const probe = await api.readStreamRange(trackId, probeStart, probeEnd)
    if (disposed || generation !== seekGeneration) return

    const syncAbs = resolveFrameSyncOffset({
      probe,
      probeStart,
      targetByte,
    })
    if (syncAbs === null) {
      fail()
      return
    }

    const appendStart = syncAbs
    const appendEnd = Math.min(total - 1, appendStart + MSE_APPEND_CHUNK - 1)
    const actualProbeEnd = probe.byteLength > 0
      ? probeStart + probe.byteLength - 1
      : probeStart - 1
    let appendBytes: Uint8Array
    const relSync = appendStart - probeStart
    if (relSync >= 0 && appendEnd <= actualProbeEnd) {
      appendBytes = probe.subarray(relSync, appendEnd - probeStart + 1)
    }
    else {
      await api.ensureStreamRange(trackId, appendStart, appendEnd)
      if (disposed || generation !== seekGeneration) return
      appendBytes = await api.readStreamRange(trackId, appendStart, appendEnd)
    }
    if (disposed || generation !== seekGeneration) return
    if (appendBytes.byteLength === 0) {
      fail()
      return
    }

    await waitForSourceBufferIdle(buffer)
    if (disposed || generation !== seekGeneration || mediaSource.readyState !== 'open') {
      return
    }

    const timeAtSync = (appendStart / total) * resolveDuration()
    try {
      if (!buffer.updating) buffer.timestampOffset = timeAtSync
    }
    catch {
      // keep going; sequence mode may already have generated timestamps
    }

    try {
      buffer.appendBuffer(new Uint8Array(appendBytes))
    }
    catch {
      fail()
      return
    }
    await waitForSourceBufferIdle(buffer)
    if (disposed || generation !== seekGeneration) return

    nextAppendOffset = appendStart + appendBytes.byteLength
    ended = false
    emitAppended()
    setAudioTimeInBuffer(buffer, clampedTime)
  }

  const pump = async () => {
    if (
      disposed
      || appendInFlight
      || discontinuityPending
      || !sourceBuffer
      || ended
    ) {
      return
    }
    if (sourceBuffer.updating || mediaSource.readyState !== 'open') return

    if (!mpegStartResolved) {
      appendInFlight = true
      try {
        const ok = await ensureMpegPayloadStart()
        if (!ok || disposed || discontinuityPending) return
      }
      finally {
        appendInFlight = false
      }
      if (!disposed && !discontinuityPending) schedulePump()
      return
    }

    const ledgerPrefix = leadingPrefixEnd(latestRanges)
    // Grow forward from the current append cursor (prefix or seek island).
    const rangeContainingCursor = latestRanges.find(
      range => nextAppendOffset >= range.start && nextAppendOffset < range.end,
    )
    const contiguousEnd = rangeContainingCursor?.end
      ?? (nextAppendOffset === 0 ? ledgerPrefix : nextAppendOffset)
    const prefixEnd = Math.max(
      contiguousEnd,
      Math.min(total, nextAppendOffset + MSE_APPEND_CHUNK),
    )
    const window = nextAppendWindow({
      nextAppendOffset,
      prefixEnd,
      total,
      maxChunk: MSE_APPEND_CHUNK,
    })
    if (!window) {
      tryEndOfStream()
      return
    }

    appendInFlight = true
    let reschedule = true
    try {
      // One retry: download finalize may rename `.part` → destination between
      // path snapshot and open; a hard fail here pauses via onError.
      let bytes: Uint8Array
      try {
        bytes = await api.readStreamRange(trackId, window.start, window.end)
      }
      catch {
        if (disposed || discontinuityPending) return
        bytes = await api.readStreamRange(trackId, window.start, window.end)
      }
      if (
        disposed
        || discontinuityPending
        || !sourceBuffer
        || mediaSource.readyState !== 'open'
      ) {
        return
      }
      if (sourceBuffer.updating) return

      let appendBytes = bytes
      let nextOffset = window.end + 1
      if (isMpegMseMime(mimeType) && window.end < total - 1) {
        const keep = completeMpegFrameByteLength(bytes)
        if (keep <= 0) {
          // No complete frame at the cursor — retrying would microtask-spin.
          fail()
          reschedule = false
          return
        }
        if (keep < bytes.byteLength) {
          appendBytes = bytes.subarray(0, keep)
          nextOffset = window.start + keep
        }
      }

      sourceBuffer.appendBuffer(new Uint8Array(appendBytes))
      // Wait for updateend before advancing the cursor — otherwise a failed
      // append leaves nextAppendOffset past bytes that never decoded.
      await waitForSourceBufferIdle(sourceBuffer)
      if (
        disposed
        || discontinuityPending
        || !sourceBuffer
        || mediaSource.readyState !== 'open'
      ) {
        return
      }
      nextAppendOffset = nextOffset
      emitAppended()
    }
    catch {
      fail()
      reschedule = false
    }
    finally {
      appendInFlight = false
      if (reschedule && !disposed && !discontinuityPending) schedulePump()
    }
  }

  const schedulePump = () => {
    if (disposed || pumpQueued || discontinuityPending) return
    pumpQueued = true
    queueMicrotask(() => {
      pumpQueued = false
      pump()
    })
  }

  const seekToTime = async (time: number) => {
    // Prefer library duration; fall back to the element once metadata lands
    // (Telegram duration can be missing → 0 at attach time).
    const seekDuration = resolveDuration()
    if (disposed || !(seekDuration > 0) || !(total > 0)) return
    const clampedTime = Math.max(0, Math.min(seekDuration, time))

    // Trust SourceBuffer, not HTMLMediaElement.buffered (stale after remove).
    if (sourceBuffer && snapTimeIntoBuffer(sourceBuffer, clampedTime) !== null) {
      setAudioTimeInBuffer(sourceBuffer, clampedTime)
      return
    }

    const generation = ++seekGeneration
    discontinuityPending = true

    try {
      while (appendInFlight && !disposed && generation === seekGeneration) {
        await new Promise(resolve => setTimeout(resolve, 16))
      }
      if (disposed || generation !== seekGeneration) return
      if (isMpegMseMime(mimeType) && !mpegStartResolved) {
        const ok = await ensureMpegPayloadStart()
        if (!ok || disposed || generation !== seekGeneration) return
      }
      if (mediaSource.readyState !== 'open') {
        if (!isMpegMseMime(mimeType)) {
          await reattachAndRebuild(clampedTime, generation)
        }
        return
      }

      const audioBytes = Math.max(1, total - mpegPayloadStart)
      const targetByte = Math.min(
        total - 1,
        mpegPayloadStart + Math.floor((clampedTime / seekDuration) * audioBytes),
      )

      const buffer = sourceBuffer
      if (!buffer || mediaSource.readyState !== 'open') return

      // WebM/MP4: grow the existing prefix — clear+rebuild closes MediaSource on WebKit.
      if (!isMpegMseMime(mimeType)) {
        ended = false
        await growUntilTime(buffer, clampedTime, generation)
        return
      }

      if (shouldRebuildFromPrefix(targetByte, latestRanges)) {
        ended = false
        const cleared = await clearBuffered(buffer)
        if (disposed || generation !== seekGeneration) return
        if (!cleared) {
          fail()
          return
        }
        await rebuildFromPrefix(buffer, clampedTime, generation)
        return
      }

      ended = false
      const cleared = await clearBuffered(buffer)
      if (disposed || generation !== seekGeneration) return
      if (!cleared) {
        fail()
        return
      }
      await appendIsland(buffer, clampedTime, targetByte, generation)
    }
    catch {
      if (generation === seekGeneration) fail()
    }
    finally {
      if (generation === seekGeneration) {
        discontinuityPending = false
        schedulePump()
      }
    }
  }

  const onSourceOpen = () => {
    if (disposed) return
    try {
      sourceBuffer = mediaSource.addSourceBuffer(mimeType)
      configureSourceBuffer(sourceBuffer)
      schedulePump()
    }
    catch {
      fail()
    }
  }

  mediaSource.addEventListener('sourceopen', onSourceOpen, { once: true })
  mediaSource.addEventListener('error', fail)
  audio.addEventListener('timeupdate', onAudioTimeUpdate)
  audio.src = objectUrl

  return {
    objectUrl,
    notifyProgress(progress: DownloadProgress) {
      if (disposed || progress.trackId !== trackId) return
      latestRanges = progress.ranges
      latestComplete = progress.complete
      schedulePump()
    },
    getAppendedOffset: () => nextAppendOffset,
    snapToBufferedTime: (time: number) => {
      if (disposed || !sourceBuffer) return null
      return snapTimeIntoBuffer(sourceBuffer, time)
    },
    landToBufferedTime: (time: number) => {
      if (disposed || !sourceBuffer) return null
      return landOnBufferedIsland(sourceBuffer, time)
    },
    seekToTime,
    dispose() {
      if (disposed) return
      disposed = true
      seekGeneration += 1
      discontinuityPending = false
      audio.removeEventListener('timeupdate', onAudioTimeUpdate)
      try {
        if (sourceBuffer && mediaSource.readyState === 'open') {
          if (sourceBuffer.updating) {
            sourceBuffer.abort()
          }
        }
      }
      catch {
        // ignore teardown races
      }
      try {
        if (mediaSource.readyState === 'open') {
          mediaSource.endOfStream()
        }
      }
      catch {
        // ignore teardown races
      }
      audio.removeAttribute('src')
      URL.revokeObjectURL(objectUrl)
    },
  }
}
