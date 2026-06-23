import { TelegramClient, Api } from "teleproto";
import bigInt from "big-integer";
import { decryptSession } from "./crypto";
import { isFileReferenceError } from "./errors";
import { parseStoredDocument } from "./document";
import type { StoredDocument } from "./document";
import { refreshSavedMusicDocumentJson } from "./refresh";
import {
  acquirePooledMtprotoClient,
  releasePooledMtprotoClient,
} from "./client-pool";

/**
 * Downloading Telegram documents (audio files and their thumbnails) over
 * MTProto, including range requests for HTTP streaming and transparent retry
 * when a cached file reference has expired.
 */

/** Telegram MTProto default per-request download chunk size (256 KiB). */
const TELEGRAM_REQUEST_SIZE = 256 * 1024;
/** Telegram requires upload.GetFile limit to be a 4 KiB multiple, at least 4 KiB. */
const MIN_CHUNK_SIZE = 4096;

function normalizeGetFileLimit(remaining: number): number {
  let limit = Math.min(TELEGRAM_REQUEST_SIZE, remaining);
  limit -= limit % MIN_CHUNK_SIZE;
  if (limit < MIN_CHUNK_SIZE) {
    limit = MIN_CHUNK_SIZE;
  }
  return limit;
}

async function getFileChunk(
  client: TelegramClient,
  dcId: number,
  location: Api.TypeInputFileLocation,
  offset: number,
  limit: number,
): Promise<Buffer> {
  const sender = await client.getSender(dcId);
  const result = await client.invokeWithSender(
    new Api.upload.GetFile({
      location,
      offset: bigInt(offset),
      limit,
    }),
    sender,
  );

  if (result instanceof Api.upload.FileCdnRedirect) {
    throw new Error("CDN downloads are not supported");
  }

  return result.bytes;
}

function buildDocumentLocation(
  data: StoredDocument,
  thumbSize = "",
): Api.InputDocumentFileLocation {
  return new Api.InputDocumentFileLocation({
    id: bigInt(data.id),
    accessHash: bigInt(data.accessHash),
    fileReference: Buffer.from(data.fileReference, "base64"),
    thumbSize,
  });
}

/**
 * Lazily downloads an inclusive byte range from Telegram, yielding chunks
 * trimmed to the exact range. Being a generator, it only fetches the next
 * Telegram block when the consumer pulls — this is what lets the HTTP response
 * apply backpressure instead of buffering the whole range in memory.
 */
async function* iterateDocumentRange(
  client: TelegramClient,
  data: StoredDocument,
  byteRange: { start: number; end: number },
  thumbSize = "",
): AsyncGenerator<Uint8Array> {
  const start = byteRange.start;
  const end = byteRange.end;
  const byteCount = end - start + 1;
  if (byteCount <= 0) {
    return;
  }
  const location = buildDocumentLocation(data, thumbSize);
  const partSize = normalizeGetFileLimit(
    Math.min(byteCount, TELEGRAM_REQUEST_SIZE),
  );
  let poolOffset = start - (start % partSize);
  let skipLeading = start - poolOffset;
  let bytesSent = 0;

  while (bytesSent < byteCount) {
    const remaining = byteCount - bytesSent;
    const chunk = await getFileChunk(
      client,
      data.dcId,
      location,
      poolOffset,
      partSize,
    );

    if (chunk.length === 0) {
      break;
    }

    const usable = skipLeading > 0 ? chunk.subarray(skipLeading) : chunk;
    skipLeading = 0;

    const trimmed
      = usable.length > remaining ? usable.subarray(0, remaining) : usable;
    if (trimmed.length > 0) {
      yield new Uint8Array(trimmed);
      bytesSent += trimmed.length;
    }

    poolOffset += partSize;

    if (chunk.length < partSize) {
      break;
    }
  }
}

/**
 * Builds a backpressure-aware {@link ReadableStream} for an inclusive byte
 * range of a stored document (or its thumbnail when `thumbSize` is set).
 *
 * The Telegram client and download iterator are created lazily on first pull
 * and torn down on completion/cancel. Crucially, the stream is driven by
 * `pull()`, so it fetches at most one Telegram block ahead of what the consumer
 * has read — when the browser pauses reading (buffer full), the download pauses
 * too instead of eagerly draining the whole range. If Telegram rejects a stale
 * file reference, the document is refreshed once and the download resumes from
 * the byte we left off at.
 */
function createDocumentRangeStream(params: {
  sessionString: string;
  storedJson: string;
  range: { start: number; end: number };
  thumbSize?: string;
  onDocumentRefreshed?: (storedJson: string) => void;
}): ReadableStream<Uint8Array> {
  const {
    sessionString,
    storedJson,
    range,
    thumbSize = "",
    onDocumentRefreshed,
  } = params;
  const byteCount = range.end - range.start + 1;

  let currentJson = storedJson;
  let client: TelegramClient | null = null;
  let iterator: AsyncGenerator<Uint8Array> | null = null;
  let emitted = 0;
  let refreshed = false;

  const newIterator = () =>
    iterateDocumentRange(
      client!,
      parseStoredDocument(currentJson),
      { start: range.start + emitted, end: range.end },
      thumbSize,
    );

  const cleanup = async () => {
    if (iterator) {
      await iterator.return(undefined).catch(() => undefined);
      iterator = null;
    }
    if (client) {
      // Release back to the pool; the connection stays open for reuse by the
      // next range window or track instead of being torn down per request.
      releasePooledMtprotoClient(sessionString);
      client = null;
    }
  };

  return new ReadableStream<Uint8Array>({
    async pull(controller) {
      try {
        if (!client) {
          client = await acquirePooledMtprotoClient(sessionString);
        }
        if (!iterator) {
          iterator = newIterator();
        }

        let result: IteratorResult<Uint8Array>;
        try {
          result = await iterator.next();
        } catch (error) {
          if (refreshed || !isFileReferenceError(error)) {
            throw error;
          }
          refreshed = true;
          currentJson = await refreshSavedMusicDocumentJson(client, currentJson);
          onDocumentRefreshed?.(currentJson);
          iterator = newIterator();
          result = await iterator.next();
        }

        if (result.done) {
          controller.close();
          await cleanup();
          return;
        }

        emitted += result.value.byteLength;
        controller.enqueue(result.value);

        if (emitted >= byteCount) {
          controller.close();
          await cleanup();
        }
      } catch (error) {
        await cleanup();
        controller.error(error);
      }
    },
    async cancel() {
      await cleanup();
    },
  });
}

export interface MtprotoDocumentStream {
  stream: ReadableStream<Uint8Array>;
  mimeType: string;
  totalSize: number;
  contentLength: number;
}

/**
 * Builds a {@link ReadableStream} for (a byte range of) a stored document. The
 * client is created lazily when the stream starts and disconnected when it ends
 * or is cancelled. If Telegram rejects a stale file reference mid-download, the
 * document is refreshed once and the download retried; `onDocumentRefreshed`
 * lets callers persist the refreshed reference.
 */
export function createMtprotoDocumentStream(
  encryptedSession: string,
  storedJson: string,
  range?: { start: number; end: number },
  onDocumentRefreshed?: (storedJson: string) => void,
): MtprotoDocumentStream {
  const data = parseStoredDocument(storedJson);
  const totalSize = Number(data.size ?? 0);
  const mimeType = data.mimeType ?? "audio/mpeg";
  const start = range?.start ?? 0;
  const end
    = range?.end ?? (totalSize > 0 ? totalSize - 1 : Number.MAX_SAFE_INTEGER); // inclusive end byte; unknown size → stream until EOF
  const byteCount = end - start + 1;

  const stream = createDocumentRangeStream({
    sessionString: decryptSession(encryptedSession),
    storedJson,
    range: { start, end },
    onDocumentRefreshed,
  });

  return { stream, mimeType, totalSize, contentLength: byteCount };
}

/**
 * Builds a {@link ReadableStream} for a stored document thumbnail. Inline
 * `thumbData` is not handled here — callers should stream that directly.
 */
export function createMtprotoThumbnailStream(
  encryptedSession: string,
  storedJson: string,
  onDocumentRefreshed?: (storedJson: string) => void,
): MtprotoDocumentStream | null {
  const stored = parseStoredDocument(storedJson);
  if (!stored.thumbSize) {
    return null;
  }

  const totalSize = Number(stored.thumbFileSize ?? 0);
  const start = 0;
  const end = totalSize > 0 ? totalSize - 1 : Number.MAX_SAFE_INTEGER;
  const byteCount = end - start + 1;

  const stream = createDocumentRangeStream({
    sessionString: decryptSession(encryptedSession),
    storedJson,
    range: { start, end },
    thumbSize: stored.thumbSize,
    onDocumentRefreshed,
  });

  return {
    stream,
    mimeType: "image/jpeg",
    totalSize,
    contentLength: byteCount,
  };
}

/**
 * Downloads a document's thumbnail. Returns inline thumb bytes immediately when
 * present, otherwise fetches the remote thumb (refreshing a stale file
 * reference once if needed). Resolves to `null` when no thumbnail exists.
 */
export async function downloadMtprotoDocumentThumbnail(
  encryptedSession: string,
  storedJson: string,
  onDocumentRefreshed?: (storedJson: string) => void,
): Promise<Buffer | null> {
  const stored = parseStoredDocument(storedJson);
  if (stored.thumbData) {
    return Buffer.from(stored.thumbData, "base64");
  }
  if (!stored.thumbSize) {
    return null;
  }

  let currentJson = storedJson;
  const sessionString = decryptSession(encryptedSession);
  const client = await acquirePooledMtprotoClient(sessionString);

  try {
    const tryDownload = async (json: string) => {
      const data = parseStoredDocument(json);
      if (!data.thumbSize) {
        return null;
      }

      const buffer = await client.downloadFile(
        buildDocumentLocation(data, data.thumbSize),
        {
          dcId: data.dcId,
          fileSize: data.thumbFileSize ? bigInt(data.thumbFileSize) : undefined,
        },
      );
      if (!buffer || (Buffer.isBuffer(buffer) && buffer.length === 0)) {
        return null;
      }
      return Buffer.isBuffer(buffer) ? buffer : Buffer.from(buffer);
    };

    try {
      return await tryDownload(currentJson);
    } catch (error) {
      if (!isFileReferenceError(error)) {
        throw error;
      }

      currentJson = await refreshSavedMusicDocumentJson(client, currentJson);
      onDocumentRefreshed?.(currentJson);
      return await tryDownload(currentJson);
    }
  } finally {
    releasePooledMtprotoClient(sessionString);
  }
}
