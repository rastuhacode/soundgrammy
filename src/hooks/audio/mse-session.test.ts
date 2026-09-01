import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { DownloadProgress } from '@/lib/api'
import {
  FakeAudioElement,
  FakeMediaSource,
  fillMpegFrames,
  flushMicrotasks,
} from './mse-session.mocks'
import {
  MSE_APPEND_CHUNK,
  attachMseSession,
  isMseTypeSupported,
  resolveMseMimeType,
  type MseFailure,
  type MseSession,
} from './mse-session'

const { readStreamRange, ensureStreamRange, backfillStreamId3 } = vi.hoisted(() => ({
  readStreamRange: vi.fn(),
  ensureStreamRange: vi.fn(),
  backfillStreamId3: vi.fn(),
}))

vi.mock('@/lib/api', () => ({
  api: {
    readStreamRange,
    ensureStreamRange,
    backfillStreamId3,
  },
}))

const TRACK_ID = 42
const TOTAL = 512 * 1024
const DURATION = 200

function id3Header(tagByteLength: number): Uint8Array {
  const payloadSize = tagByteLength - 10
  return new Uint8Array([
    0x49,
    0x44,
    0x33,
    0x04,
    0,
    0,
    (payloadSize >> 21) & 0x7F,
    (payloadSize >> 14) & 0x7F,
    (payloadSize >> 7) & 0x7F,
    payloadSize & 0x7F,
  ])
}

async function waitFor(predicate: () => boolean): Promise<void> {
  await vi.waitFor(
    () => {
      expect(predicate()).toBe(true)
    },
    { timeout: 2000, interval: 5 },
  )
}

function stubMediaSource(options?: {
  total?: number
  duration?: number
  sourceBufferInit?: FakeMediaSource['sourceBufferInit']
  instances?: FakeMediaSource[]
}) {
  const total = options?.total ?? TOTAL
  const duration = options?.duration ?? DURATION
  const instances = options?.instances
  const sourceBufferInit = options?.sourceBufferInit ?? {}

  vi.stubGlobal('MediaSource', class extends FakeMediaSource {
    constructor() {
      super()
      this.configure(total, duration)
      this.sourceBufferInit = { ...sourceBufferInit }
      instances?.push(this)
    }

    static isTypeSupported(): boolean {
      return true
    }
  })
}

describe('resolveMseMimeType / isMseTypeSupported', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('returns null when MediaSource is unavailable', () => {
    vi.stubGlobal('MediaSource', undefined)
    expect(resolveMseMimeType('audio/mpeg')).toBeNull()
    expect(isMseTypeSupported('audio/mpeg')).toBe(false)
  })

  it('picks the first supported candidate for webm / mp4 / mpeg', () => {
    const supported = new Set([
      'audio/webm; codecs="opus"',
      'audio/mp4; codecs="mp4a.40.2"',
      'audio/mpeg',
    ])
    vi.stubGlobal('MediaSource', class {
      static isTypeSupported(type: string): boolean {
        return supported.has(type)
      }
    })

    expect(resolveMseMimeType('audio/webm')).toBe('audio/webm; codecs="opus"')
    expect(resolveMseMimeType('audio/m4a')).toBe('audio/mp4; codecs="mp4a.40.2"')
    expect(resolveMseMimeType('audio/mp3')).toBe('audio/mpeg')
    expect(isMseTypeSupported('audio/ogg')).toBe(false)
  })

  it('falls back through webm codec candidates', () => {
    const supported = new Set(['audio/webm'])
    vi.stubGlobal('MediaSource', class {
      static isTypeSupported(type: string): boolean {
        return supported.has(type)
      }
    })

    expect(resolveMseMimeType('video/webm')).toBe('audio/webm')
  })
})

describe('attachMseSession', () => {
  const objectUrls = new Map<string, FakeMediaSource>()
  let urlSeq = 0
  let mediaSourceInstances: FakeMediaSource[] = []
  let session: MseSession | null = null
  let audio: FakeAudioElement

  beforeEach(() => {
    objectUrls.clear()
    urlSeq = 0
    mediaSourceInstances = []
    session = null
    readStreamRange.mockReset()
    ensureStreamRange.mockReset()
    backfillStreamId3.mockReset()
    ensureStreamRange.mockResolvedValue(undefined)
    backfillStreamId3.mockResolvedValue(undefined)

    // Default: any inclusive window returns MPEG-framed bytes of the right length.
    readStreamRange.mockImplementation(
      async (_trackId: number, _sessionId: string, start: number, end: number) => {
        const length = Math.max(0, end - start + 1)
        return fillMpegFrames(length)
      },
    )

    stubMediaSource({ instances: mediaSourceInstances })

    vi.spyOn(URL, 'createObjectURL').mockImplementation((obj: object) => {
      const url = `blob:mse-test-${++urlSeq}`
      if (obj instanceof FakeMediaSource) objectUrls.set(url, obj)
      return url
    })
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation((url: string) => {
      objectUrls.delete(url)
    })

    audio = new FakeAudioElement(objectUrls)
  })

  afterEach(() => {
    session?.dispose()
    session = null
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  function progress(
    partial: Partial<DownloadProgress> & Pick<DownloadProgress, 'ranges'>,
  ): DownloadProgress {
    const received = partial.ranges.reduce(
      (sum, range) => sum + Math.max(0, range.end - range.start),
      0,
    )
    return {
      trackId: TRACK_ID,
      received,
      total: TOTAL,
      complete: false,
      ...partial,
    }
  }

  async function attach(options?: {
    mimeType?: string
    duration?: number
    total?: number
    alwaysFailAppend?: boolean
    holdAppends?: boolean
    maxBufferedBytes?: number
    onAppendedOffset?: (offset: number) => void
    onBufferedChanged?: () => void
    onError?: (failure: MseFailure) => void
    bufferAheadSeconds?: number
  }): Promise<{ session: MseSession, mediaSource: FakeMediaSource }> {
    const mimeType = options?.mimeType ?? 'audio/mpeg'
    const total = options?.total ?? TOTAL
    const duration = options?.duration ?? DURATION

    stubMediaSource({
      total,
      duration,
      instances: mediaSourceInstances,
      sourceBufferInit: {
        alwaysFailAppend: options?.alwaysFailAppend,
        holdAppends: options?.holdAppends,
        maxBufferedBytes: options?.maxBufferedBytes,
      },
    })

    const next = attachMseSession({
      audio: audio as unknown as HTMLAudioElement,
      trackId: TRACK_ID,
      sessionId: 'test-session',
      mimeType,
      total,
      duration,
      bufferAheadSeconds: options?.bufferAheadSeconds ?? Number.POSITIVE_INFINITY,
      onAppendedOffset: options?.onAppendedOffset,
      onBufferedChanged: options?.onBufferedChanged,
      onError: options?.onError,
    })
    session = next

    await waitFor(() => mediaSourceInstances.length > 0
      && mediaSourceInstances[mediaSourceInstances.length - 1]!.readyState === 'open'
      && mediaSourceInstances[mediaSourceInstances.length - 1]!.sourceBuffers.length === 1)

    const mediaSource = mediaSourceInstances[mediaSourceInstances.length - 1]!
    return { session: next, mediaSource }
  }

  async function pumpPrefix(
    active: MseSession,
    prefixEnd: number,
    complete = false,
  ): Promise<void> {
    active.notifyProgress(progress({
      ranges: [{ start: 0, end: prefixEnd }],
      complete,
      received: prefixEnd,
    }))
    await waitFor(() => active.getAppendedOffset() > 0)
    await flushMicrotasks(16)
  }

  it('opens MediaSource, uses sequence mode for mpeg, and pumps the leading prefix', async () => {
    const appended: number[] = []
    const { session: active, mediaSource } = await attach({
      onAppendedOffset: offset => appended.push(offset),
    })

    expect(mediaSource.sourceBuffers[0]!.mode).toBe('sequence')
    expect(audio.src).toMatch(/^blob:mse-test-/)

    await pumpPrefix(active, MSE_APPEND_CHUNK * 2)

    expect(active.getAppendedOffset()).toBeGreaterThan(0)
    expect(appended.at(-1)).toBe(active.getAppendedOffset())
    expect(readStreamRange).toHaveBeenCalled()
    // MPEG cold start resolves ID3 / Xing before the first append window.
    expect(readStreamRange.mock.calls[0]).toEqual([TRACK_ID, 'test-session', 0, 9])
    expect(readStreamRange.mock.calls.some(
      call => call[2] === 0 && call[3] === MSE_APPEND_CHUNK - 1,
    )).toBe(true)
  })

  it('bounds read-ahead and resumes pumping as playback advances', async () => {
    const total = MSE_APPEND_CHUNK * 8
    const { session: active } = await attach({
      total,
      bufferAheadSeconds: 32,
    })

    await waitFor(() => active.getAppendedOffset() > MSE_APPEND_CHUNK)
    await flushMicrotasks(16)
    const stoppedOffset = active.getAppendedOffset()
    expect(stoppedOffset).toBeLessThan(total)

    audio.tickTime(30)
    await waitFor(() => active.getAppendedOffset() > stoppedOffset)
  })

  it('backfills a large ID3 prefix only after playable audio is continuous', async () => {
    const total = MSE_APPEND_CHUNK * 6
    const tagEnd = MSE_APPEND_CHUNK * 2 + 10
    readStreamRange.mockImplementation(
      async (_trackId: number, _sessionId: string, start: number, end: number) => {
        if (start === 0 && end === 9) return id3Header(tagEnd)
        return fillMpegFrames(Math.max(0, end - start + 1))
      },
    )
    const { session: active } = await attach({ total })

    // A seek island reaches EOF but leaves an unheard audio gap.
    active.notifyProgress(progress({
      total,
      received: MSE_APPEND_CHUNK * 4,
      ranges: [
        { start: 0, end: MSE_APPEND_CHUNK },
        { start: MSE_APPEND_CHUNK * 2, end: MSE_APPEND_CHUNK * 4 },
        { start: MSE_APPEND_CHUNK * 5, end: total },
      ],
    }))
    await flushMicrotasks(16)
    expect(backfillStreamId3).not.toHaveBeenCalled()

    active.notifyProgress(progress({
      total,
      received: MSE_APPEND_CHUNK * 5,
      ranges: [
        { start: 0, end: MSE_APPEND_CHUNK },
        { start: MSE_APPEND_CHUNK * 2, end: total },
      ],
    }))
    await waitFor(() => backfillStreamId3.mock.calls.length === 1)
    expect(backfillStreamId3).toHaveBeenCalledWith(
      TRACK_ID,
      'test-session',
    )
  })

  it('does not backfill ID3 when the whole file is already complete', async () => {
    const total = MSE_APPEND_CHUNK * 4
    const tagEnd = MSE_APPEND_CHUNK + 10
    readStreamRange.mockImplementation(
      async (_trackId: number, _sessionId: string, start: number, end: number) => {
        if (start === 0 && end === 9) return id3Header(tagEnd)
        return fillMpegFrames(Math.max(0, end - start + 1))
      },
    )
    const { session: active } = await attach({ total })
    active.notifyProgress(progress({
      total,
      received: total,
      ranges: [{ start: 0, end: total }],
      complete: true,
    }))
    await flushMicrotasks(16)
    expect(backfillStreamId3).not.toHaveBeenCalled()
  })

  it('allows media end-of-stream while ID3 storage backfill is pending', async () => {
    const total = MSE_APPEND_CHUNK * 5
    const tagEnd = MSE_APPEND_CHUNK * 2 + 10
    readStreamRange.mockImplementation(
      async (_trackId: number, _sessionId: string, start: number, end: number) => {
        if (start === 0 && end === 9) return id3Header(tagEnd)
        return fillMpegFrames(Math.max(0, end - start + 1))
      },
    )
    backfillStreamId3.mockImplementation(() => new Promise(() => {}))
    const { session: active, mediaSource } = await attach({ total })
    await waitFor(() => active.getAppendedOffset() >= total)

    active.notifyProgress(progress({
      total,
      received: total - MSE_APPEND_CHUNK,
      ranges: [
        { start: 0, end: MSE_APPEND_CHUNK },
        { start: MSE_APPEND_CHUNK * 2, end: total },
      ],
      complete: false,
    }))
    await waitFor(() => mediaSource.endOfStreamCalls === 1)
    expect(backfillStreamId3).toHaveBeenCalledOnce()
  })

  it('retries a transient ID3 backfill failure on a later playback tick', async () => {
    const total = MSE_APPEND_CHUNK * 5
    const tagEnd = MSE_APPEND_CHUNK * 2 + 10
    readStreamRange.mockImplementation(
      async (_trackId: number, _sessionId: string, start: number, end: number) => {
        if (start === 0 && end === 9) return id3Header(tagEnd)
        return fillMpegFrames(Math.max(0, end - start + 1))
      },
    )
    backfillStreamId3
      .mockRejectedValueOnce(new Error('temporary network error'))
      .mockResolvedValueOnce(undefined)
    const now = Date.now()
    const { session: active } = await attach({ total })
    active.notifyProgress(progress({
      total,
      received: total - MSE_APPEND_CHUNK,
      ranges: [
        { start: 0, end: MSE_APPEND_CHUNK },
        { start: MSE_APPEND_CHUNK * 2, end: total },
      ],
    }))
    await waitFor(() => backfillStreamId3.mock.calls.length === 1)
    await flushMicrotasks(16)

    vi.spyOn(Date, 'now').mockReturnValue(now + 10_000)
    audio.tickTime(1)
    await waitFor(() => backfillStreamId3.mock.calls.length === 2)
  })

  it('deduplicates ID3 backfill while the storage request is in flight', async () => {
    const total = MSE_APPEND_CHUNK * 5
    const tagEnd = MSE_APPEND_CHUNK * 2 + 10
    readStreamRange.mockImplementation(
      async (_trackId: number, _sessionId: string, start: number, end: number) => {
        if (start === 0 && end === 9) return id3Header(tagEnd)
        return fillMpegFrames(Math.max(0, end - start + 1))
      },
    )
    backfillStreamId3.mockImplementation(() => new Promise(() => {}))
    const { session: active } = await attach({ total })
    const audioReady = progress({
      total,
      received: total - MSE_APPEND_CHUNK,
      ranges: [
        { start: 0, end: MSE_APPEND_CHUNK },
        { start: MSE_APPEND_CHUNK * 2, end: total },
      ],
    })

    active.notifyProgress(audioReady)
    active.notifyProgress(audioReady)
    audio.tickTime(1)
    await flushMicrotasks(16)
    expect(backfillStreamId3).toHaveBeenCalledOnce()
  })

  it('does not start ID3 backfill after the MSE session is disposed', async () => {
    const total = MSE_APPEND_CHUNK * 5
    const tagEnd = MSE_APPEND_CHUNK * 2 + 10
    readStreamRange.mockImplementation(
      async (_trackId: number, _sessionId: string, start: number, end: number) => {
        if (start === 0 && end === 9) return id3Header(tagEnd)
        return fillMpegFrames(Math.max(0, end - start + 1))
      },
    )
    const { session: active } = await attach({ total })
    active.dispose()
    active.notifyProgress(progress({
      total,
      received: total - MSE_APPEND_CHUNK,
      ranges: [
        { start: 0, end: MSE_APPEND_CHUNK },
        { start: MSE_APPEND_CHUNK * 2, end: total },
      ],
    }))
    await flushMicrotasks(16)
    expect(backfillStreamId3).not.toHaveBeenCalled()
  })

  it('uses segments mode for non-mpeg mime types', async () => {
    const { mediaSource } = await attach({ mimeType: 'audio/webm; codecs="opus"' })
    expect(mediaSource.sourceBuffers[0]!.mode).toBe('segments')
  })

  it('calls endOfStream only after the full file is appended and complete', async () => {
    const smallTotal = MSE_APPEND_CHUNK
    const { session: active, mediaSource } = await attach({ total: smallTotal })

    active.notifyProgress(progress({
      ranges: [{ start: 0, end: smallTotal }],
      complete: false,
      received: smallTotal,
      total: smallTotal,
    }))
    await waitFor(() => active.getAppendedOffset() >= smallTotal)
    await flushMicrotasks(8)
    expect(mediaSource.endOfStreamCalls).toBe(0)
    expect(backfillStreamId3).not.toHaveBeenCalled()

    active.notifyProgress(progress({
      ranges: [{ start: 0, end: smallTotal }],
      complete: true,
      received: smallTotal,
      total: smallTotal,
    }))
    // currentTime is 0 → EOS is allowed immediately (nothing to clamp mid-track).
    await waitFor(() => mediaSource.endOfStreamCalls === 1)
    expect(mediaSource.readyState).toBe('ended')
  })

  it('defers endOfStream while the playhead is mid-track after full load', async () => {
    const onError = vi.fn()
    const smallTotal = MSE_APPEND_CHUNK
    const { session: active, mediaSource } = await attach({
      total: smallTotal,
      onError,
    })

    // Simulate listening while bytes finish appending.
    audio.currentTime = 40

    active.notifyProgress(progress({
      ranges: [{ start: 0, end: smallTotal }],
      complete: true,
      received: smallTotal,
      total: smallTotal,
    }))
    await waitFor(() => active.getAppendedOffset() >= smallTotal)
    await flushMicrotasks(16)

    // Full file is in the SourceBuffer, but EOS must wait — WebKit clamps
    // duration on EOS and can fire `ended` / pause mid-track.
    expect(mediaSource.endOfStreamCalls).toBe(0)
    expect(mediaSource.readyState).toBe('open')
    expect(onError).not.toHaveBeenCalled()

    // Near the end → EOS proceeds so the element can fire a real `ended`.
    audio.tickTime(DURATION - 0.5)
    await flushMicrotasks(8)
    expect(mediaSource.endOfStreamCalls).toBe(1)
    expect(mediaSource.readyState).toBe('ended')
    expect(onError).not.toHaveBeenCalled()
  })

  it('retries a transient readStreamRange failure without calling onError', async () => {
    const onError = vi.fn()
    let appendAttempts = 0
    readStreamRange.mockImplementation(
      async (_trackId: number, _sessionId: string, start: number, end: number) => {
        const length = Math.max(0, end - start + 1)
        // Payload-start probes are small; the append pump retries only the
        // leading window after MPEG start is resolved.
        if (length >= MSE_APPEND_CHUNK) {
          appendAttempts += 1
          if (appendAttempts === 1) {
            throw new Error('ENOENT: partial renamed during finalize')
          }
        }
        return fillMpegFrames(length)
      },
    )

    const { session: active } = await attach({ onError })
    await pumpPrefix(active, MSE_APPEND_CHUNK)

    expect(onError).not.toHaveBeenCalled()
    expect(active.getAppendedOffset()).toBeGreaterThan(0)
    expect(appendAttempts).toBeGreaterThanOrEqual(2)
  })

  it('does not call onError when endOfStream throws InvalidStateError', async () => {
    const onError = vi.fn()
    const smallTotal = MSE_APPEND_CHUNK
    const { session: active, mediaSource } = await attach({
      total: smallTotal,
      onError,
    })

    mediaSource.endOfStream = () => {
      mediaSource.endOfStreamCalls += 1
      throw new DOMException('InvalidStateError')
    }

    active.notifyProgress(progress({
      ranges: [{ start: 0, end: smallTotal }],
      complete: true,
      received: smallTotal,
      total: smallTotal,
    }))
    await waitFor(() => active.getAppendedOffset() >= smallTotal)
    await flushMicrotasks(16)

    expect(mediaSource.endOfStreamCalls).toBeGreaterThanOrEqual(1)
    expect(onError).not.toHaveBeenCalled()
    // Remains open so a later retry can still finish the stream.
    expect(mediaSource.readyState).toBe('open')
  })

  it('does not advance the append cursor when appendBuffer throws', async () => {
    const onError = vi.fn()
    const { session: active } = await attach({
      onError,
      alwaysFailAppend: true,
    })

    await waitFor(() => onError.mock.calls.length >= 1)
    expect(active.getAppendedOffset()).toBe(0)
    expect(onError).toHaveBeenCalledWith(expect.objectContaining({
      stage: 'append-pump',
      cause: expect.anything(),
    }))
  })

  it('recovers Windows quota pressure without reporting a fatal session error', async () => {
    const onError = vi.fn()
    const total = MSE_APPEND_CHUNK * 8
    const { session: active, mediaSource } = await attach({
      total,
      maxBufferedBytes: MSE_APPEND_CHUNK,
      onError,
    })
    const buffer = mediaSource.sourceBuffers[0]!

    await waitFor(() => buffer.quotaExceededCalls === 1)
    const stalledOffset = active.getAppendedOffset()

    expect(stalledOffset).toBeGreaterThan(0)
    expect(stalledOffset).toBeLessThan(total)
    expect(onError).not.toHaveBeenCalled()
    expect(buffer.removeCalls).toHaveLength(0)

    // More download progress must not spin on the same full buffer.
    active.notifyProgress(progress({
      ranges: [{ start: 0, end: total }],
      received: total,
      total,
      complete: true,
    }))
    await flushMicrotasks(16)

    expect(buffer.quotaExceededCalls).toBe(1)
    expect(active.getAppendedOffset()).toBe(stalledOffset)

    // Playback creates safe back-buffer space. The session evicts it and
    // resumes from the same append cursor instead of pausing the player.
    audio.tickTime(20)
    await waitFor(() => active.getAppendedOffset() > stalledOffset)

    expect(buffer.removeCalls.length).toBeGreaterThan(0)
    expect(onError).not.toHaveBeenCalled()
  })

  it('snaps and lands onto SourceBuffer ranges (not element buffered)', async () => {
    const { session: active, mediaSource } = await attach()
    await pumpPrefix(active, MSE_APPEND_CHUNK)
    const buffer = mediaSource.sourceBuffers[0]!
    // Force a known island regardless of proportional math.
    buffer.buffered.setSingle(10, 25)

    expect(active.snapToBufferedTime(12)).toBe(12)
    expect(active.snapToBufferedTime(9.2)).toBe(10)
    expect(active.snapToBufferedTime(100)).toBeNull()

    // Land is more permissive than snap for VBR skew.
    expect(active.landToBufferedTime(12)).toBe(12)
    expect(active.landToBufferedTime(8)).toBe(10)
    expect(active.landToBufferedTime(30)).toBe(24.95)
  })

  it('seekToTime into an already-buffered range only sets currentTime', async () => {
    const { session: active, mediaSource } = await attach()
    await pumpPrefix(active, MSE_APPEND_CHUNK * 2)
    mediaSource.sourceBuffers[0]!.buffered.setSingle(0, 40)
    const ensureCallsBefore = ensureStreamRange.mock.calls.length

    await active.seekToTime(15)

    expect(audio.currentTime).toBe(15)
    expect(ensureStreamRange.mock.calls.length).toBe(ensureCallsBefore)
    expect(mediaSource.sourceBuffers[0]!.removeCalls).toHaveLength(0)
  })

  it('MP3 seek inside the leading download prefix rebuilds from byte 0', async () => {
    const { session: active, mediaSource } = await attach()
    await pumpPrefix(active, TOTAL / 2)
    // Pretend playhead is still near the start while target is deeper in prefix.
    mediaSource.sourceBuffers[0]!.buffered.setSingle(0, 5)

    const targetTime = 40
    await active.seekToTime(targetTime)

    const buffer = mediaSource.sourceBuffers[0]!
    expect(buffer.removeCalls.length).toBeGreaterThan(0)
    expect(ensureStreamRange).toHaveBeenCalled()
    // Rebuild starts from byte 0 — first ensure after seek should be near 0.
    const ensureAfterSeek = ensureStreamRange.mock.calls.filter(
      call => call[0] === TRACK_ID,
    )
    expect(ensureAfterSeek.some(call => call[2] === 0)).toBe(true)
    expect(active.getAppendedOffset()).toBeGreaterThan(0)
  })

  it('MP3 seek outside the leading prefix appends a discontinuous island', async () => {
    const { session: active, mediaSource } = await attach()
    // Only a short leading prefix — seek far past it.
    await pumpPrefix(active, 32 * 1024)
    mediaSource.sourceBuffers[0]!.buffered.setSingle(0, 2)

    const targetTime = 150
    await active.seekToTime(targetTime)

    const buffer = mediaSource.sourceBuffers[0]!
    expect(buffer.removeCalls.length).toBeGreaterThan(0)
    expect(ensureStreamRange).toHaveBeenCalled()

    const targetByte = Math.floor((targetTime / DURATION) * TOTAL)
    const probeStarts = ensureStreamRange.mock.calls.map(call => call[2] as number)
    // Probe window starts ~4KiB before the target byte (or 0).
    expect(probeStarts.some(start => start <= targetByte && start >= targetByte - 4096 - 1))
      .toBe(true)

    // Island append should move the cursor past the leading prefix.
    expect(active.getAppendedOffset()).toBeGreaterThan(32 * 1024)
    // currentTime should land on the island.
    expect(audio.currentTime).toBeGreaterThan(0)
  })

  it('WebM seek grows the existing prefix without clear+rebuild', async () => {
    const bigTotal = 8 * 1024 * 1024
    const { session: active, mediaSource } = await attach({
      mimeType: 'audio/webm; codecs="opus"',
      total: bigTotal,
      holdAppends: true,
    })
    const buffer = mediaSource.sourceBuffers[0]!
    // Release the held open-pump without committing bytes so offset stays 0.
    mediaSource.readyState = 'ended'
    buffer.discardHeldAppend()
    buffer.dispatchEvent(new Event('updateend'))
    await flushMicrotasks(16)
    mediaSource.readyState = 'open'
    buffer.buffered.clear()

    const removeBefore = buffer.removeCalls.length
    ensureStreamRange.mockClear()

    await active.seekToTime(30)

    expect(buffer.removeCalls.length).toBe(removeBefore)
    expect(ensureStreamRange).toHaveBeenCalled()
    expect(active.getAppendedOffset()).toBeGreaterThan(0)
    expect(buffer.buffered.ranges.some(
      range => range.start <= 30 && 30 < range.end,
    )).toBe(true)
  })

  it('MP3 island seek sets timestampOffset from the sync byte position', async () => {
    const { session: active, mediaSource } = await attach()
    await pumpPrefix(active, 16 * 1024)
    mediaSource.sourceBuffers[0]!.buffered.setSingle(0, 1)

    const targetTime = 150
    await active.seekToTime(targetTime)

    const buffer = mediaSource.sourceBuffers[0]!
    // After append the mock resets timestampOffset to 0; the island must have
    // landed near the proportional seek time (not back at 0).
    expect(buffer.buffered.ranges.length).toBe(1)
    const island = buffer.buffered.ranges[0]!
    expect(island.start).toBeGreaterThan(100)
    expect(audio.currentTime).toBeGreaterThanOrEqual(island.start)
    expect(audio.currentTime).toBeLessThan(island.end)
  })

  it('does not endOfStream while a discontinuity seek is pending', async () => {
    const smallTotal = MSE_APPEND_CHUNK
    const { session: active, mediaSource } = await attach({
      total: smallTotal,
      holdAppends: true,
    })
    const buffer = mediaSource.sourceBuffers[0]!
    mediaSource.readyState = 'ended'
    buffer.discardHeldAppend()
    buffer.dispatchEvent(new Event('updateend'))
    await flushMicrotasks(8)
    mediaSource.readyState = 'open'

    // Hold the island append so discontinuityPending stays true.
    buffer.holdAppends = true
    const seekPromise = active.seekToTime(80)
    await flushMicrotasks(16)

    active.notifyProgress(progress({
      ranges: [{ start: 0, end: smallTotal }],
      complete: true,
      received: smallTotal,
      total: smallTotal,
    }))
    await flushMicrotasks(16)
    expect(mediaSource.endOfStreamCalls).toBe(0)

    buffer.holdAppends = false
    buffer.completeAppend()
    await seekPromise
  })

  it('WebM seek with a closed MediaSource reattaches and rebuilds', async () => {
    const { session: active, mediaSource } = await attach({
      mimeType: 'audio/webm; codecs="opus"',
    })
    await pumpPrefix(active, MSE_APPEND_CHUNK)
    mediaSource.forceClose()
    mediaSource.sourceBuffers[0]!.buffered.clear()

    active.notifyProgress(progress({
      ranges: [{ start: 0, end: TOTAL }],
      received: TOTAL,
    }))

    await active.seekToTime(20)

    expect(mediaSourceInstances.length).toBeGreaterThan(1)
    const latest = mediaSourceInstances[mediaSourceInstances.length - 1]!
    expect(latest.readyState).toBe('open')
    expect(active.getAppendedOffset()).toBeGreaterThan(0)
  })

  it('ignores progress for a different trackId', async () => {
    const { session: active } = await attach({ holdAppends: true })
    await flushMicrotasks(8)
    const readsBefore = readStreamRange.mock.calls.length
    const offsetBefore = active.getAppendedOffset()

    active.notifyProgress({
      trackId: TRACK_ID + 1,
      received: TOTAL,
      total: TOTAL,
      ranges: [{ start: 0, end: TOTAL }],
      complete: true,
    })
    await flushMicrotasks(16)

    expect(active.getAppendedOffset()).toBe(offsetBefore)
    expect(readStreamRange.mock.calls.length).toBe(readsBefore)
  })

  it('ignores stale progress with a different file size', async () => {
    const { session: active } = await attach({ holdAppends: true })
    await flushMicrotasks(8)
    const readsBefore = readStreamRange.mock.calls.length
    const offsetBefore = active.getAppendedOffset()

    active.notifyProgress({
      trackId: TRACK_ID,
      received: TOTAL + 1,
      total: TOTAL + 1,
      ranges: [{ start: 0, end: TOTAL + 1 }],
      complete: true,
    })
    await flushMicrotasks(16)

    expect(active.getAppendedOffset()).toBe(offsetBefore)
    expect(readStreamRange.mock.calls.length).toBe(readsBefore)
    expect(backfillStreamId3).not.toHaveBeenCalled()
  })

  it('dispose cancels further pumps and revokes the object URL', async () => {
    const { session: active, mediaSource } = await attach({ holdAppends: true })
    await flushMicrotasks(8)
    const buffer = mediaSource.sourceBuffers[0]!
    const url = active.objectUrl
    const offsetAtDispose = active.getAppendedOffset()
    active.dispose()

    buffer.holdAppends = false
    buffer.completeAppend()
    active.notifyProgress(progress({
      ranges: [{ start: 0, end: TOTAL }],
      complete: true,
    }))
    await flushMicrotasks(16)

    expect(active.getAppendedOffset()).toBe(offsetAtDispose)
    expect(objectUrls.has(url)).toBe(false)
    expect(audio.src).toBe('')
    expect(audio.loadCalls).toBe(1)
    expect(mediaSource.sourceBuffers).toHaveLength(0)
    expect(active.snapToBufferedTime(0)).toBeNull()
  })

  it('a newer seek supersedes an in-flight seek (generation guard)', async () => {
    const { session: active, mediaSource } = await attach()
    await pumpPrefix(active, 16 * 1024)
    mediaSource.sourceBuffers[0]!.buffered.setSingle(0, 1)

    // Hold the first island append so the second seek can bump generation.
    mediaSource.sourceBuffers[0]!.holdAppends = true

    const first = active.seekToTime(160)
    await flushMicrotasks(16)

    mediaSource.sourceBuffers[0]!.holdAppends = false
    mediaSource.sourceBuffers[0]!.completeAppend()

    // Second seek while first may still be finishing clear/probe.
    await active.seekToTime(170)
    await first

    // Session should still be healthy and reflect the later seek cursor.
    expect(active.getAppendedOffset()).toBeGreaterThan(0)
    expect(mediaSource.sourceBuffers[0]!.removeCalls.length).toBeGreaterThan(0)
  })

  it('seekToTime is a no-op while duration is unknown', async () => {
    const { session: active, mediaSource } = await attach({ duration: 0 })
    // Cold-start MPEG probes may ensure the ID3/header window; seek itself must not.
    ensureStreamRange.mockClear()
    await active.seekToTime(30)
    expect(mediaSource.sourceBuffers[0]!.removeCalls).toHaveLength(0)
    expect(ensureStreamRange).not.toHaveBeenCalled()
  })

  it('fails the session when island frame sync cannot be resolved', async () => {
    const onError = vi.fn()
    // Pump with real frames first; MPEG appends require frame-aligned bytes.
    const { session: active, mediaSource } = await attach({ onError })
    await pumpPrefix(active, 8 * 1024)
    mediaSource.sourceBuffers[0]!.buffered.setSingle(0, 1)

    // Island seek probe / append sees only junk — no MPEG sync words.
    readStreamRange.mockImplementation(
      async (_trackId: number, _sessionId: string, start: number, end: number) => {
        return new Uint8Array(Math.max(0, end - start + 1))
      },
    )

    await active.seekToTime(150)
    expect(onError).toHaveBeenCalled()
  })

  it('fails when SourceBuffer.remove throws during an MP3 discontinuity seek', async () => {
    const onError = vi.fn()
    const { session: active, mediaSource } = await attach({ onError })
    await pumpPrefix(active, 16 * 1024)
    const buffer = mediaSource.sourceBuffers[0]!
    buffer.buffered.setSingle(0, 1)
    buffer.failNextRemove = true

    const appendsBefore = buffer.appendHistory.length
    await active.seekToTime(150)

    expect(onError).toHaveBeenCalled()
    // Must not rebuild/append onto the uncleared buffer.
    expect(buffer.appendHistory.length).toBe(appendsBefore)
    expect(buffer.buffered.ranges).toEqual([{ start: 0, end: 1 }])
  })

  it('fails when WebM grow stalls without covering the seek time', async () => {
    const onError = vi.fn()
    const { session: active, mediaSource } = await attach({
      mimeType: 'audio/webm; codecs="opus"',
      onError,
    })
    await pumpPrefix(active, MSE_APPEND_CHUNK)
    const buffer = mediaSource.sourceBuffers[0]!
    buffer.buffered.setSingle(0, 2)
    buffer.freezeBuffered = true

    active.notifyProgress(progress({
      ranges: [{ start: 0, end: TOTAL }],
      received: TOTAL,
    }))

    await active.seekToTime(80)

    expect(onError).toHaveBeenCalled()
    // Must not snap the far target onto the early buffer tip.
    expect(audio.currentTime).toBeLessThan(5)
  })

  it('pauses the pump while a discontinuity seek is in flight', async () => {
    const { session: active, mediaSource } = await attach()
    await pumpPrefix(active, 16 * 1024)
    mediaSource.sourceBuffers[0]!.buffered.setSingle(0, 1)
    mediaSource.sourceBuffers[0]!.holdAppends = true

    const seekPromise = active.seekToTime(160)
    await flushMicrotasks(8)

    const offsetDuringSeek = active.getAppendedOffset()
    // Extra progress must not race ahead of the discontinuity.
    active.notifyProgress(progress({
      ranges: [{ start: 0, end: TOTAL }],
      received: TOTAL,
    }))
    await flushMicrotasks(16)
    expect(active.getAppendedOffset()).toBe(offsetDuringSeek)

    mediaSource.sourceBuffers[0]!.holdAppends = false
    mediaSource.sourceBuffers[0]!.completeAppend()
    await seekPromise
  })
})
