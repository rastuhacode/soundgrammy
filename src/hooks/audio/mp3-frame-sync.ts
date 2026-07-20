/** MPEG-1/2 Layer I–III frame sync helpers for discontinuous MSE appends. */

/**
 * Total bytes occupied by an ID3v2 header+tag at the start of `header`
 * (10-byte header + synchsafe size + optional footer), or `null` if absent.
 *
 * Large embedded album art (multi‑MB APIC frames) must be skipped before
 * feeding `audio/mpeg` to MSE — WebKit will not build buffered ranges from
 * tag bytes and eventually closes the MediaSource.
 */
export function id3v2TagByteLength(header: Uint8Array): number | null {
  if (header.length < 10) return null
  if (header[0] !== 0x49 || header[1] !== 0x44 || header[2] !== 0x33) {
    return null
  }
  const flags = header[5]!
  const size
    = ((header[6]! & 0x7f) << 21)
      | ((header[7]! & 0x7f) << 14)
      | ((header[8]! & 0x7f) << 7)
      | (header[9]! & 0x7f)
  const footer = (flags & 0x10) !== 0 ? 10 : 0
  const total = 10 + size + footer
  if (!(total >= 10) || !Number.isFinite(total)) return null
  return total
}

/**
 * Byte offset where MPEG frames begin. Prefers a parsed ID3v2 size; falls
 * back to scanning for a validated frame sync inside `probe`.
 */
export function resolveMpegPayloadStart(
  probe: Uint8Array,
  fileTotal: number,
): number {
  const tagLen = id3v2TagByteLength(probe)
  if (tagLen !== null && tagLen < fileTotal) {
    return tagLen
  }
  const sync = findMp3FrameOffset(probe, 0)
  return sync >= 0 ? sync : 0
}

const BITRATES_V1_L3 = [
  0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
]
const BITRATES_V1_L2 = [
  0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 0,
]
const BITRATES_V1_L1 = [
  0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 0,
]
const BITRATES_V2_L3 = [
  0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0,
]
const BITRATES_V2_L1 = [
  0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256, 0,
]
const SAMPLE_RATES_V1 = [44100, 48000, 32000, 0]
const SAMPLE_RATES_V2 = [22050, 24000, 16000, 0]
const SAMPLE_RATES_V25 = [11025, 12000, 8000, 0]

export interface Mp3FrameInfo {
  offset: number
  size: number
  sampleRate: number
  bitrate: number
}

function bitrateTable(versionBits: number, layerBits: number): number[] | null {
  // version: 3=MPEG1, 2=MPEG2, 0=MPEG2.5; layer: 1=III, 2=II, 3=I
  if (versionBits === 1 || layerBits === 0) return null
  if (versionBits === 3) {
    if (layerBits === 1) return BITRATES_V1_L3
    if (layerBits === 2) return BITRATES_V1_L2
    return BITRATES_V1_L1
  }
  if (layerBits === 3) return BITRATES_V2_L1
  return BITRATES_V2_L3
}

function sampleRateTable(versionBits: number): number[] | null {
  if (versionBits === 3) return SAMPLE_RATES_V1
  if (versionBits === 2) return SAMPLE_RATES_V2
  if (versionBits === 0) return SAMPLE_RATES_V25
  return null
}

export function parseMp3FrameAt(
  data: Uint8Array,
  offset: number,
): Mp3FrameInfo | null {
  if (offset + 4 > data.length) return null
  if (data[offset] !== 0xFF || (data[offset + 1]! & 0xE0) !== 0xE0) return null

  const b1 = data[offset + 1]!
  const b2 = data[offset + 2]!
  const versionBits = (b1 >> 3) & 0x03
  const layerBits = (b1 >> 1) & 0x03
  const bitrateIndex = (b2 >> 4) & 0x0F
  const sampleRateIndex = (b2 >> 2) & 0x03
  const padding = (b2 >> 1) & 0x01

  if (bitrateIndex === 0 || bitrateIndex === 15 || sampleRateIndex === 3) {
    return null
  }

  const bitrates = bitrateTable(versionBits, layerBits)
  const rates = sampleRateTable(versionBits)
  if (!bitrates || !rates) return null

  const bitrate = bitrates[bitrateIndex]!
  const sampleRate = rates[sampleRateIndex]!
  if (!(bitrate > 0 && sampleRate > 0)) return null

  let size: number
  if (layerBits === 3) {
    // Layer I
    size = Math.floor(((12 * bitrate * 1000) / sampleRate + padding) * 4)
  }
  else if (versionBits === 3) {
    // MPEG1 Layer II/III
    size = Math.floor((144 * bitrate * 1000) / sampleRate) + padding
  }
  else {
    // MPEG2/2.5 Layer II/III
    size = Math.floor((72 * bitrate * 1000) / sampleRate) + padding
  }

  if (size < 24 || size > 4096) return null
  return { offset, size, sampleRate, bitrate }
}

/**
 * Length of a leading run of complete MPEG frames in `data`.
 * Trailing mid-frame bytes are excluded so MSE appends stay frame-aligned
 * (WebKit's audio/mpeg demuxer can garble the start of a segment that ends
 * mid-frame).
 */
export function completeMpegFrameByteLength(data: Uint8Array): number {
  let offset = 0
  let lastComplete = 0
  while (offset + 4 <= data.length) {
    const frame = parseMp3FrameAt(data, offset)
    if (!frame || frame.offset !== offset) break
    if (offset + frame.size > data.length) break
    offset += frame.size
    lastComplete = offset
  }
  return lastComplete
}

/**
 * Find the first validated MPEG frame at or after `fromIndex`.
 * Prefers a candidate whose following frame also parses when enough bytes exist.
 */
export function findMp3FrameOffset(
  data: Uint8Array,
  fromIndex = 0,
): number {
  const start = Math.max(0, Math.min(fromIndex, data.length))
  for (let i = start; i + 4 < data.length; i++) {
    const frame = parseMp3FrameAt(data, i)
    if (!frame) continue
    const next = i + frame.size
    if (next + 4 <= data.length) {
      if (parseMp3FrameAt(data, next)) return i
      continue
    }
    // Near end of probe window — accept a single valid header.
    if (next <= data.length) return i
  }
  return -1
}

/**
 * Absolute file offset of the MPEG frame to append for `targetByte`.
 * Prefers the last validated frame that starts at or before the target
 * (so mid-frame seeks snap to the enclosing frame, not the next one).
 */
export function resolveFrameSyncOffset(options: {
  probe: Uint8Array
  probeStart: number
  targetByte: number
}): number | null {
  const { probe, probeStart, targetByte } = options
  const relativeTarget = Math.max(0, targetByte - probeStart)
  let lastAtOrBefore = -1
  let i = 0

  while (i + 4 < probe.length) {
    const frame = parseMp3FrameAt(probe, i)
    if (!frame) {
      i += 1
      continue
    }

    const next = frame.offset + frame.size
    const validated = next + 4 <= probe.length
      ? parseMp3FrameAt(probe, next) !== null
      : next <= probe.length
    if (!validated) {
      i += 1
      continue
    }

    if (frame.offset <= relativeTarget) {
      lastAtOrBefore = frame.offset
      i = next
      continue
    }

    if (lastAtOrBefore >= 0) return probeStart + lastAtOrBefore
    return probeStart + frame.offset
  }

  if (lastAtOrBefore >= 0) return probeStart + lastAtOrBefore
  return null
}
