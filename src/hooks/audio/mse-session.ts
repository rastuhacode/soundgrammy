import { api, type DownloadProgress } from '@/lib/api'
import {
  leadingPrefixEnd,
  nextAppendWindow,
  shouldEndOfStream,
  shouldRebuildFromPrefix,
  type ByteRange,
} from './mse-append-queue'
import { resolveFrameSyncOffset } from './mp3-frame-sync'

/** Matches Rust streaming::CHUNK_SIZE — keep append IPC payloads bounded. */
export const MSE_APPEND_CHUNK = 128 * 1024

/** Bytes before the seek target to search for a frame sync. */
const SEEK_PROBE_BACK = 4 * 1024

export function isMseTypeSupported(mimeType: string): boolean {
  return typeof MediaSource !== 'undefined'
    && typeof MediaSource.isTypeSupported === 'function'
    && MediaSource.isTypeSupported(mimeType)
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

function bufferCoversTime(buffer: SourceBuffer, time: number): boolean {
  for (let i = 0; i < buffer.buffered.length; i++) {
    if (time >= buffer.buffered.start(i) && time < buffer.buffered.end(i)) {
      return true
    }
  }
  return false
}

function snapTimeIntoBuffer(
  buffer: SourceBuffer,
  time: number,
  tolerance = 1.5,
): number | null {
  if (buffer.buffered.length === 0) return null
  let best: { snapped: number, distance: number } | null = null
  for (let i = 0; i < buffer.buffered.length; i++) {
    const start = buffer.buffered.start(i)
    const end = buffer.buffered.end(i)
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
  if (buffer.buffered.length === 0) return null
  // Proportional byte mapping can miss by more than 1.5s on VBR — still land.
  const start = buffer.buffered.start(0)
  const end = buffer.buffered.end(buffer.buffered.length - 1)
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

  const mediaSource = new MediaSource()
  const objectUrl = URL.createObjectURL(mediaSource)
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
    try {
      mediaSource.endOfStream()
      ended = true
    }
    catch {
      fail()
    }
  }

  const onSourceBufferUpdateEnd = () => {
    if (disposed) return
    onBufferedChanged?.()
    tryEndOfStream()
    schedulePump()
  }

  const configureSourceBuffer = (buffer: SourceBuffer) => {
    try {
      buffer.mode = 'sequence'
    }
    catch {
      // ignore unsupported mode
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

  const clearBuffered = async (buffer: SourceBuffer) => {
    await waitForSourceBufferIdle(buffer)
    if (disposed || mediaSource.readyState !== 'open') return
    if (buffer.updating) {
      try {
        buffer.abort()
      }
      catch {
        // ignore
      }
      await waitForSourceBufferIdle(buffer)
    }
    if (buffer.buffered.length === 0) return
    const bufferedEnd = buffer.buffered.end(buffer.buffered.length - 1)
    const removeEnd = Number.isFinite(mediaSource.duration) && mediaSource.duration > 0
      ? Math.max(mediaSource.duration, bufferedEnd)
      : bufferedEnd + 1
    try {
      buffer.remove(0, removeEnd)
    }
    catch {
      buffer.remove(buffer.buffered.start(0), bufferedEnd)
    }
    await waitForSourceBufferIdle(buffer)
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
    buffer.appendBuffer(new Uint8Array(bytes))
    await waitForSourceBufferIdle(buffer)
    if (disposed || generation !== seekGeneration) return false
    nextAppendOffset = end + 1
    emitAppended()
    return true
  }

  /** Rebuild timeline from byte 0 until `clampedTime` is inside buffered. */
  const rebuildFromPrefix = async (
    buffer: SourceBuffer,
    clampedTime: number,
    generation: number,
  ) => {
    const prefixEnd = leadingPrefixEnd(latestRanges)
    nextAppendOffset = 0
    ended = false
    try {
      buffer.timestampOffset = 0
    }
    catch {
      try {
        buffer.mode = 'sequence'
        buffer.timestampOffset = 0
      }
      catch {
        fail()
        return
      }
    }
    emitAppended()

    while (
      !disposed
      && generation === seekGeneration
      && nextAppendOffset < prefixEnd
      && !bufferCoversTime(buffer, clampedTime)
    ) {
      const window = nextAppendWindow({
        nextAppendOffset,
        prefixEnd,
        total,
        maxChunk: MSE_APPEND_CHUNK,
      })
      if (!window) break
      const ok = await appendChunk(buffer, window.start, window.end, generation)
      if (!ok) return
    }

    if (disposed || generation !== seekGeneration) return
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
      buffer.timestampOffset = timeAtSync
    }
    catch {
      try {
        buffer.mode = 'sequence'
        buffer.timestampOffset = timeAtSync
      }
      catch {
        fail()
        return
      }
    }

    buffer.appendBuffer(new Uint8Array(appendBytes))
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
    try {
      const bytes = await api.readStreamRange(trackId, window.start, window.end)
      if (
        disposed
        || discontinuityPending
        || !sourceBuffer
        || mediaSource.readyState !== 'open'
      ) {
        return
      }
      if (sourceBuffer.updating) return
      sourceBuffer.appendBuffer(bytes)
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
      nextAppendOffset = window.end + 1
      emitAppended()
    }
    catch {
      fail()
    }
    finally {
      appendInFlight = false
      if (!disposed && !discontinuityPending) schedulePump()
    }
  }

  const schedulePump = () => {
    if (disposed || pumpQueued || discontinuityPending) return
    pumpQueued = true
    queueMicrotask(() => {
      pumpQueued = false
      void pump()
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
      if (mediaSource.readyState !== 'open') return

      const targetByte = Math.min(
        total - 1,
        Math.floor((clampedTime / seekDuration) * total),
      )

      if (shouldRebuildFromPrefix(targetByte, latestRanges)) {
        // Clear in place and rebuild from byte 0. Recreating SourceBuffer on
        // WebKit leaves HTMLMediaElement.buffered stale and skips resume.
        const buffer = sourceBuffer
        if (!buffer || mediaSource.readyState !== 'open') return
        await clearBuffered(buffer)
        if (disposed || generation !== seekGeneration) return
        await rebuildFromPrefix(buffer, clampedTime, generation)
        return
      }

      // Forward / mid-file island: clear in place. Recreating SourceBuffer on
      // WebKit leaves HTMLMediaElement.buffered stale and freezes playback.
      const buffer = sourceBuffer
      if (!buffer || mediaSource.readyState !== 'open') return
      await clearBuffered(buffer)
      if (disposed || generation !== seekGeneration) return
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
