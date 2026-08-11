/**
 * Minimal MediaSource / SourceBuffer stand-ins for mse-session tests.
 * Time coverage is proportional: appendedBytes / totalBytes * duration.
 */

export class FakeTimeRanges {
  ranges: Array<{ start: number, end: number }> = []

  get length(): number {
    return this.ranges.length
  }

  start(index: number): number {
    const range = this.ranges[index]
    if (!range) throw new DOMException('IndexSizeError')
    return range.start
  }

  end(index: number): number {
    const range = this.ranges[index]
    if (!range) throw new DOMException('IndexSizeError')
    return range.end
  }

  clear() {
    this.ranges = []
  }

  setSingle(start: number, end: number) {
    this.ranges = end > start ? [{ start, end }] : []
  }

  merge(start: number, end: number) {
    if (!(end > start)) return
    this.ranges.push({ start, end })
    this.ranges.sort((a, b) => a.start - b.start)
    const merged: Array<{ start: number, end: number }> = []
    for (const range of this.ranges) {
      const last = merged[merged.length - 1]
      if (!last || range.start > last.end + 0.001) {
        merged.push({ ...range })
      }
      else {
        last.end = Math.max(last.end, range.end)
      }
    }
    this.ranges = merged
  }
}

export class FakeSourceBuffer extends EventTarget {
  updating = false
  mode: 'sequence' | 'segments' = 'segments'
  timestampOffset = 0
  buffered = new FakeTimeRanges()
  appendHistory: Uint8Array[] = []
  removeCalls: Array<{ start: number, end: number }> = []
  abortCalls = 0
  failNextAppend = false
  /**
   * When true, the next appendBuffer throws once, then switches to holdAppends
   * so the session pump cannot microtask-spin forever on persistent failures.
   */
  alwaysFailAppend = false
  /** When true, appendBuffer leaves updating=true until completeAppend(). */
  holdAppends = false
  /** When true, successful appends do not extend buffered time (stagnant growth). */
  freezeBuffered = false
  /** Simulates Chromium/WebView2 rejecting appends once its MSE quota is full. */
  maxBufferedBytes = Number.POSITIVE_INFINITY
  quotaExceededCalls = 0
  /** When true, the next remove() throws once. */
  failNextRemove = false
  private pendingComplete: (() => void) | null = null
  private retainedBufferedBytes = 0

  constructor(
    readonly totalBytes: number,
    readonly mediaDuration: number,
  ) {
    super()
  }

  appendBuffer(data: BufferSource) {
    if (this.updating) {
      throw new DOMException('InvalidStateError')
    }
    if (this.alwaysFailAppend) {
      this.alwaysFailAppend = false
      this.holdAppends = true
      throw new DOMException('Synthetic append failure', 'OperationError')
    }
    if (this.failNextAppend) {
      this.failNextAppend = false
      throw new DOMException('QuotaExceededError')
    }

    const bytes = toUint8Array(data)
    if (this.retainedBufferedBytes + bytes.byteLength > this.maxBufferedBytes) {
      this.quotaExceededCalls += 1
      throw new DOMException(
        'The SourceBuffer is full, and cannot free space to append additional buffers.',
        'QuotaExceededError',
      )
    }
    this.appendHistory.push(bytes)
    this.retainedBufferedBytes += bytes.byteLength
    this.updating = true

    const finish = () => {
      if (!this.freezeBuffered) {
        const span = this.totalBytes > 0 && this.mediaDuration > 0
          ? (bytes.byteLength / this.totalBytes) * this.mediaDuration
          : 0
        const start = this.timestampOffset
        const end = start + span
        // Island / rebuild: replace coverage. Sequential growth: merge forward.
        if (this.buffered.ranges.length === 0 || this.timestampOffset > 0.001) {
          this.buffered.setSingle(start, end)
        }
        else {
          const last = this.buffered.ranges[this.buffered.ranges.length - 1]!
          this.buffered.merge(last.end, last.end + span)
        }
        // After a successful contiguous append, reset offset so the next pump
        // continues from the buffered tip (sequence-mode growth).
        this.timestampOffset = 0
      }
      this.updating = false
      this.pendingComplete = null
      this.dispatchEvent(new Event('updateend'))
    }

    if (this.holdAppends) {
      this.pendingComplete = finish
      return
    }
    queueMicrotask(finish)
  }

  completeAppend() {
    this.pendingComplete?.()
  }

  /** Drop a held append without committing buffered ranges (test helper). */
  discardHeldAppend() {
    this.pendingComplete = null
    this.updating = false
    this.holdAppends = false
  }

  remove(start: number, end: number) {
    if (this.failNextRemove) {
      this.failNextRemove = false
      throw new DOMException('InvalidStateError')
    }
    if (this.updating) {
      throw new DOMException('InvalidStateError')
    }
    this.removeCalls.push({ start, end })
    this.updating = true
    queueMicrotask(() => {
      this.buffered.clear()
      this.retainedBufferedBytes = 0
      this.timestampOffset = 0
      this.updating = false
      this.dispatchEvent(new Event('updateend'))
    })
  }

  abort() {
    this.abortCalls += 1
    this.updating = false
    this.pendingComplete = null
    this.dispatchEvent(new Event('updateend'))
  }
}

export class FakeMediaSource extends EventTarget {
  readyState: 'closed' | 'open' | 'ended' = 'closed'
  duration = Number.NaN
  sourceBuffers: FakeSourceBuffer[] = []
  endOfStreamCalls = 0
  closedForSeek = false
  private totalBytes = 0
  private mediaDuration = 0
  /** Applied to every SourceBuffer created by addSourceBuffer. */
  sourceBufferInit: {
    alwaysFailAppend?: boolean
    holdAppends?: boolean
    maxBufferedBytes?: number
  } = {}

  /** Configure proportional time mapping before sourceopen. */
  configure(totalBytes: number, mediaDuration: number) {
    this.totalBytes = totalBytes
    this.mediaDuration = mediaDuration
  }

  addSourceBuffer(mimeType: string): FakeSourceBuffer {
    void mimeType
    if (this.readyState !== 'open') {
      throw new DOMException('InvalidStateError')
    }
    const buffer = new FakeSourceBuffer(this.totalBytes, this.mediaDuration)
    if (this.sourceBufferInit.alwaysFailAppend) buffer.alwaysFailAppend = true
    if (this.sourceBufferInit.holdAppends) buffer.holdAppends = true
    if (this.sourceBufferInit.maxBufferedBytes != null) {
      buffer.maxBufferedBytes = this.sourceBufferInit.maxBufferedBytes
    }
    this.sourceBuffers.push(buffer)
    return buffer
  }

  removeSourceBuffer(buffer: FakeSourceBuffer) {
    const index = this.sourceBuffers.indexOf(buffer)
    if (index >= 0) this.sourceBuffers.splice(index, 1)
  }

  endOfStream() {
    if (this.readyState !== 'open') {
      throw new DOMException('InvalidStateError')
    }
    this.endOfStreamCalls += 1
    this.readyState = 'ended'
  }

  /** Simulate a WebKit close mid-session (non-MPEG reattach path). */
  forceClose() {
    this.readyState = 'closed'
    this.closedForSeek = true
  }
}

export class FakeAudioElement extends EventTarget {
  currentTime = 0
  duration = Number.NaN
  loadCalls = 0
  private _src = ''
  private readonly objectUrls: Map<string, FakeMediaSource>

  constructor(objectUrls: Map<string, FakeMediaSource>) {
    super()
    this.objectUrls = objectUrls
  }

  get src(): string {
    return this._src
  }

  set src(value: string) {
    this._src = value
    const mediaSource = this.objectUrls.get(value)
    if (!mediaSource || mediaSource.readyState !== 'closed') return
    queueMicrotask(() => {
      if (this._src !== value) return
      mediaSource.readyState = 'open'
      mediaSource.dispatchEvent(new Event('sourceopen'))
    })
  }

  removeAttribute(name: string) {
    if (name === 'src') this._src = ''
  }

  load() {
    this.loadCalls += 1
  }

  /** Test helper: advance the playhead and fire timeupdate (MSE EOS deferral). */
  tickTime(time: number) {
    this.currentTime = time
    this.dispatchEvent(new Event('timeupdate'))
  }
}

/** MPEG1 Layer III, 128 kbps, 44100 Hz, no padding — frame size 417. */
export function mpegFrameHeader(): Uint8Array {
  return new Uint8Array([0xFF, 0xFB, 0x90, 0x00])
}

/** Fill a byte window with repeating valid MPEG frames (and optional junk prefix). */
export function fillMpegFrames(
  length: number,
  options: { junkPrefix?: number } = {},
): Uint8Array {
  const junkPrefix = options.junkPrefix ?? 0
  const header = mpegFrameHeader()
  const frameSize = 417
  const data = new Uint8Array(length)
  let offset = junkPrefix
  while (offset + 4 <= length) {
    data.set(header.subarray(0, Math.min(4, length - offset)), offset)
    offset += frameSize
  }
  return data
}

export async function flushMicrotasks(times = 8): Promise<void> {
  for (let i = 0; i < times; i++) {
    await Promise.resolve()
  }
}

function toUint8Array(data: BufferSource): Uint8Array {
  if (data instanceof ArrayBuffer) return new Uint8Array(data)
  if (ArrayBuffer.isView(data)) {
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
  }
  return new Uint8Array()
}
