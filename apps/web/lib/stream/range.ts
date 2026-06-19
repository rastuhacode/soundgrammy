/**
 * HTTP byte-range helpers for audio streaming.
 *
 * The audio route serves the full requested range in a single response and
 * relies on stream backpressure to avoid over-fetching from Telegram. The
 * native `<audio>` element reads only its current position plus its own
 * look-ahead, then stops reading until playback advances, and issues a fresh
 * `Range` request only when the listener seeks.
 */

export interface ByteRange {
  start: number;
  end: number;
}

export type ParsedByteRange = ByteRange | "unsatisfiable" | null;

/**
 * Parses an HTTP `Range` header against a known file size.
 *
 * Returns the resolved inclusive byte range, the string `"unsatisfiable"` when
 * the range can't be served (caller should answer 416), or `null` when the
 * header isn't a byte range we understand (caller should serve the full file).
 */
export function parseByteRange(
  rangeHeader: string,
  fileSize: number,
): ParsedByteRange {
  if (!rangeHeader.startsWith("bytes=")) {
    return null;
  }

  const [startStr, endStr] = rangeHeader.slice(6).split("-");
  if (startStr === "" && endStr === "") {
    return "unsatisfiable";
  }

  let start: number;
  let end: number;

  if (startStr === "") {
    const suffixLength = Number(endStr);
    if (!Number.isFinite(suffixLength) || suffixLength <= 0) {
      return "unsatisfiable";
    }
    start = Math.max(0, fileSize - suffixLength);
    end = fileSize - 1;
  } else {
    start = Number(startStr);
    end = endStr !== "" ? Number(endStr) : fileSize - 1;
  }

  if (!Number.isFinite(start) || !Number.isFinite(end)) {
    return "unsatisfiable";
  }
  if (start >= fileSize || start > end) {
    return "unsatisfiable";
  }

  end = Math.min(end, fileSize - 1);
  return { start, end };
}
