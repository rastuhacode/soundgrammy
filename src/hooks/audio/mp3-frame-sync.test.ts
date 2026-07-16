import { describe, expect, it } from 'vitest'
import {
  findMp3FrameOffset,
  parseMp3FrameAt,
  resolveFrameSyncOffset,
} from './mp3-frame-sync'

/** MPEG1 Layer III, 128 kbps, 44100 Hz, no padding — frame size 417. */
function frameHeader(): Uint8Array {
  return new Uint8Array([0xFF, 0xFB, 0x90, 0x00])
}

function twoFramesWithPrefix(prefixLen: number): Uint8Array {
  const header = frameHeader()
  const frameSize = 417
  const data = new Uint8Array(prefixLen + frameSize * 2)
  data.set(header, prefixLen)
  data.set(header, prefixLen + frameSize)
  return data
}

describe('parseMp3FrameAt', () => {
  it('parses a standard MPEG1 Layer III header', () => {
    const data = frameHeader()
    const frame = parseMp3FrameAt(data, 0)
    expect(frame).toMatchObject({
      offset: 0,
      size: 417,
      sampleRate: 44100,
      bitrate: 128,
    })
  })

  it('rejects non-sync bytes', () => {
    expect(parseMp3FrameAt(new Uint8Array([0x00, 0xFB, 0x90, 0x00]), 0)).toBeNull()
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

  it('returns null when no frame exists', () => {
    expect(resolveFrameSyncOffset({
      probe: new Uint8Array(64).fill(0),
      probeStart: 0,
      targetByte: 10,
    })).toBeNull()
  })
})
