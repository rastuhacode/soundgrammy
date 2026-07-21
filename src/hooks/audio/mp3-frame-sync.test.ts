import { describe, expect, it } from 'vitest'
import {
  completeMpegFrameByteLength,
  estimateMpegDurationSeconds,
  findMp3FrameOffset,
  id3v2TagByteLength,
  parseMp3FrameAt,
  resolveFrameSyncOffset,
  resolveMpegPayloadStart,
} from './mp3-frame-sync'

/** MPEG1 Layer III, 128 kbps, 44100 Hz, stereo, no padding — frame size 417. */
function frameHeader(bitrateIndex = 0x9): Uint8Array {
  // 0xFF 0xFB: MPEG1 Layer III, no CRC
  // bitrate in high nibble of byte 2; sample rate 44100 (bits 3-2 = 00); no pad
  return new Uint8Array([0xFF, 0xFB, (bitrateIndex << 4) & 0xF0, 0x00])
}

function twoFramesWithPrefix(prefixLen: number): Uint8Array {
  const header = frameHeader()
  const frameSize = 417
  const data = new Uint8Array(prefixLen + frameSize * 2)
  data.set(header, prefixLen)
  data.set(header, prefixLen + frameSize)
  return data
}

/** Pack N identical CBR frames into a buffer. */
function cbrFrames(count: number, bitrateIndex = 0x9): Uint8Array {
  const header = frameHeader(bitrateIndex)
  const frame = parseMp3FrameAt(header, 0)!
  const data = new Uint8Array(frame.size * count)
  for (let i = 0; i < count; i++) {
    data.set(header, i * frame.size)
  }
  return data
}

describe('id3v2TagByteLength', () => {
  it('returns null when the header is missing or truncated', () => {
    expect(id3v2TagByteLength(new Uint8Array(9))).toBeNull()
    expect(id3v2TagByteLength(new Uint8Array([0xFF, 0xFB, 0x90, 0x00]))).toBeNull()
  })

  it('decodes synchsafe size including the 10-byte header', () => {
    // size body = 0x00 0x00 0x02 0x01 → 257 bytes; total skip = 267
    const header = new Uint8Array([
      0x49, 0x44, 0x33, 0x03, 0x00, 0x00, 0x00, 0x00, 0x02, 0x01,
    ])
    expect(id3v2TagByteLength(header)).toBe(10 + 257)
  })

  it('includes the footer when the footer flag is set', () => {
    const header = new Uint8Array([
      0x49, 0x44, 0x33, 0x04, 0x00, 0x10, 0x00, 0x00, 0x00, 0x10,
    ])
    expect(id3v2TagByteLength(header)).toBe(10 + 16 + 10)
  })

  it('handles multi-megabyte album-art tags (broken-track repro)', () => {
    // synchsafe 0x01 0x08 0x61 0x4e → 2_240_718 body bytes (track 153)
    const header = new Uint8Array([
      0x49, 0x44, 0x33, 0x04, 0x00, 0x00, 0x01, 0x08, 0x61, 0x4e,
    ])
    expect(id3v2TagByteLength(header)).toBe(10 + 2_240_718)
  })
})

describe('resolveMpegPayloadStart', () => {
  it('skips an ID3v2 tag when present', () => {
    const probe = new Uint8Array(64)
    probe.set([0x49, 0x44, 0x33, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20], 0)
    expect(resolveMpegPayloadStart(probe, 10_000)).toBe(10 + 0x20)
  })

  it('falls back to frame sync when there is no ID3 tag', () => {
    const data = twoFramesWithPrefix(12)
    expect(resolveMpegPayloadStart(data, data.length)).toBe(12)
  })
})

describe('parseMp3FrameAt', () => {
  it('parses a standard MPEG1 Layer III header', () => {
    const data = frameHeader()
    const frame = parseMp3FrameAt(data, 0)
    expect(frame).toMatchObject({
      offset: 0,
      size: 417,
      sampleRate: 44100,
      bitrate: 128,
      versionBits: 3,
      layerBits: 1,
      channelMode: 0,
    })
  })

  it('rejects non-sync bytes', () => {
    expect(parseMp3FrameAt(new Uint8Array([0x00, 0xFB, 0x90, 0x00]), 0)).toBeNull()
  })

  it('rejects truncated headers and reserved bitrate / sample-rate indexes', () => {
    expect(parseMp3FrameAt(new Uint8Array([0xFF, 0xFB, 0x90]), 0)).toBeNull()
    // bitrate index 0 (free) and 15 (bad)
    expect(parseMp3FrameAt(new Uint8Array([0xFF, 0xFB, 0x00, 0x00]), 0)).toBeNull()
    expect(parseMp3FrameAt(new Uint8Array([0xFF, 0xFB, 0xF0, 0x00]), 0)).toBeNull()
  })

  it('accounts for padding in the frame size', () => {
    // Same as frameHeader but padding bit set → size 418.
    const padded = new Uint8Array([0xFF, 0xFB, 0x92, 0x00])
    expect(parseMp3FrameAt(padded, 0)).toMatchObject({ size: 418 })
  })
})

describe('estimateMpegDurationSeconds', () => {
  it('estimates CBR duration from size and constant bitrate', () => {
    const probe = cbrFrames(12)
    const frame = parseMp3FrameAt(probe, 0)!
    const audioBytes = frame.size * 1000
    const fileTotal = 100 + audioBytes
    const seconds = estimateMpegDurationSeconds({
      fileTotal,
      payloadStart: 100,
      payloadProbe: probe,
    })
    expect(seconds).toBeCloseTo((audioBytes * 8) / (128 * 1000), 6)
  })

  it('uses Xing frame count when present', () => {
    // MPEG1 Layer III stereo → Xing at offset 36.
    const frameSize = 417
    const probe = new Uint8Array(frameSize)
    probe.set(frameHeader(), 0)
    probe.set([0x58, 0x69, 0x6e, 0x67], 36) // Xing
    // flags: frames present
    probe[40] = 0
    probe[41] = 0
    probe[42] = 0
    probe[43] = 0x01
    // 1000 frames
    probe[44] = 0
    probe[45] = 0
    probe[46] = 0x03
    probe[47] = 0xe8
    const seconds = estimateMpegDurationSeconds({
      fileTotal: 500_000,
      payloadStart: 0,
      payloadProbe: probe,
    })
    // 1000 * 1152 / 44100
    expect(seconds).toBeCloseTo((1000 * 1152) / 44100, 6)
  })

  it('returns null when consecutive frames disagree on bitrate (VBR)', () => {
    const low = frameHeader(0x9) // 128
    const high = frameHeader(0xe) // 320 — size 1044 at 44100
    const lowFrame = parseMp3FrameAt(low, 0)!
    const highFrame = parseMp3FrameAt(high, 0)!
    const probe = new Uint8Array(lowFrame.size + highFrame.size)
    probe.set(low, 0)
    probe.set(high, lowFrame.size)
    expect(estimateMpegDurationSeconds({
      fileTotal: probe.length * 100,
      payloadStart: 0,
      payloadProbe: probe,
    })).toBeNull()
  })

  it('returns null when the probe does not start on a frame', () => {
    expect(estimateMpegDurationSeconds({
      fileTotal: 10_000,
      payloadStart: 0,
      payloadProbe: twoFramesWithPrefix(12),
    })).toBeNull()
  })
})

describe('completeMpegFrameByteLength', () => {
  it('keeps only whole frames and drops a trailing partial frame', () => {
    const header = frameHeader()
    const frameSize = 417
    const data = new Uint8Array(frameSize * 2 + 50)
    data.set(header, 0)
    data.set(header, frameSize)
    data.set(header, frameSize * 2) // incomplete: only 50 bytes of third frame
    expect(completeMpegFrameByteLength(data)).toBe(frameSize * 2)
  })

  it('returns 0 when the buffer does not start on a frame', () => {
    const data = twoFramesWithPrefix(12)
    expect(completeMpegFrameByteLength(data)).toBe(0)
  })
})

describe('findMp3FrameOffset', () => {
  it('skips junk and finds a frame validated by the next header', () => {
    const data = twoFramesWithPrefix(37)
    expect(findMp3FrameOffset(data, 0)).toBe(37)
  })

  it('finds a frame at or after the target index', () => {
    const data = twoFramesWithPrefix(10)
    expect(findMp3FrameOffset(data, 10)).toBe(10)
    expect(findMp3FrameOffset(data, 11)).toBe(10 + 417)
  })

  it('accepts a single valid header near the end of the probe window', () => {
    const header = frameHeader()
    const data = new Uint8Array(417)
    data.set(header, 0)
    expect(findMp3FrameOffset(data, 0)).toBe(0)
  })

  it('returns -1 when nothing validates', () => {
    expect(findMp3FrameOffset(new Uint8Array(64).fill(0xAB), 0)).toBe(-1)
  })
})

describe('resolveFrameSyncOffset', () => {
  it('maps a mid-frame target back to the enclosing frame in the probe', () => {
    const probeStart = 1000
    const data = twoFramesWithPrefix(20)
    const sync = resolveFrameSyncOffset({
      probe: data,
      probeStart,
      targetByte: probeStart + 20 + 50,
    })
    expect(sync).toBe(probeStart + 20)
  })

  it('prefers the last frame at or before the target over the next frame', () => {
    const probeStart = 5000
    const data = twoFramesWithPrefix(0)
    const sync = resolveFrameSyncOffset({
      probe: data,
      probeStart,
      targetByte: probeStart + 417 + 10,
    })
    expect(sync).toBe(probeStart + 417)
  })

  it('returns the first frame after the target when none start before it', () => {
    const probeStart = 0
    const data = twoFramesWithPrefix(80)
    const sync = resolveFrameSyncOffset({
      probe: data,
      probeStart,
      targetByte: 10,
    })
    expect(sync).toBe(80)
  })

  it('returns null when no frame exists', () => {
    expect(resolveFrameSyncOffset({
      probe: new Uint8Array(64).fill(0),
      probeStart: 0,
      targetByte: 10,
    })).toBeNull()
  })
})
