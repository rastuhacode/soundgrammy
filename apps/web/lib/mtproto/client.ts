import { TelegramClient, sessions, Api } from "telegram";
import bigInt from "big-integer";
import { getMtprotoCredentials } from "./config";
import { decryptSession, encryptSession } from "./crypto";
import { isFileReferenceError, parseStoredDocument } from "./document";
import type { StoredDocument } from "./document";
import { refreshSavedMusicDocumentJson } from "./refresh";

const TELEGRAM_REQUEST_SIZE = 256 * 1024;

export { parseStoredDocument, type StoredDocument } from "./document";

export function saveClientSession(client: TelegramClient): string {
  return encryptSession((client.session as sessions.StringSession).save());
}

export async function createMtprotoClient(
  sessionString = "",
): Promise<TelegramClient> {
  const { apiId, apiHash } = getMtprotoCredentials();
  const client = new TelegramClient(
    new sessions.StringSession(sessionString),
    apiId,
    apiHash,
    {
      connectionRetries: 3,
    },
  );
  await client.connect();
  return client;
}

export async function withMtprotoClient<T>(
  encryptedSession: string,
  fn: (client: TelegramClient) => Promise<T>,
): Promise<T> {
  const sessionString = decryptSession(encryptedSession);
  const client = await createMtprotoClient(sessionString);
  try {
    return await fn(client);
  } finally {
    await client.disconnect();
  }
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

async function downloadDocumentRange(
  client: TelegramClient,
  data: StoredDocument,
  byteRange: { start: number; end: number },
  onChunk: (chunk: Uint8Array) => void,
): Promise<void> {
  const totalSize = Number(data.size ?? 0);
  const start = byteRange.start;
  const end = byteRange.end;
  const byteCount = end - start + 1;
  const location = buildDocumentLocation(data);
  const chunkCount = Math.ceil(byteCount / TELEGRAM_REQUEST_SIZE);

  const iter = client.iterDownload({
    file: location,
    offset: bigInt(start),
    limit: chunkCount,
    requestSize: TELEGRAM_REQUEST_SIZE,
    chunkSize: TELEGRAM_REQUEST_SIZE,
    fileSize: totalSize > 0 ? bigInt(totalSize) : undefined,
    dcId: data.dcId,
  });

  let bytesSent = 0;
  for await (const chunk of iter) {
    const buffer = chunk as Buffer;
    const remaining = byteCount - bytesSent;
    if (remaining <= 0) {
      break;
    }

    if (buffer.length > remaining) {
      onChunk(new Uint8Array(buffer.subarray(0, remaining)));
      return;
    }

    onChunk(new Uint8Array(buffer));
    bytesSent += buffer.length;
  }
}

export interface MtprotoDocumentStream {
  stream: ReadableStream<Uint8Array>;
  mimeType: string;
  totalSize: number;
  contentLength: number;
}

export function createMtprotoDocumentStream(
  encryptedSession: string,
  storedJson: string,
  range?: { start: number; end: number },
  onDocumentRefreshed?: (storedJson: string) => void,
): MtprotoDocumentStream {
  let currentJson = storedJson;
  const data = parseStoredDocument(currentJson);
  const totalSize = Number(data.size ?? 0);
  const mimeType = data.mimeType ?? "audio/mpeg";
  const start = range?.start ?? 0;
  const end =
    range?.end ??
    (totalSize > 0 ? totalSize - 1 : Number.MAX_SAFE_INTEGER);
  const byteCount = end - start + 1;
  const sessionString = decryptSession(encryptedSession);
  let client: TelegramClient | null = null;

  const stream = new ReadableStream<Uint8Array>({
    async start(controller) {
      try {
        client = await createMtprotoClient(sessionString);
        const byteRange = { start, end };

        const tryDownload = async (json: string) => {
          await downloadDocumentRange(
            client!,
            parseStoredDocument(json),
            byteRange,
            (chunk) => controller.enqueue(chunk),
          );
        };

        try {
          await tryDownload(currentJson);
        } catch (error) {
          if (!isFileReferenceError(error)) {
            throw error;
          }

          currentJson = await refreshSavedMusicDocumentJson(
            client!,
            currentJson,
          );
          onDocumentRefreshed?.(currentJson);
          await tryDownload(currentJson);
        }

        controller.close();
      } catch (error) {
        controller.error(error);
      } finally {
        if (client) {
          await client.disconnect();
          client = null;
        }
      }
    },
    async cancel() {
      if (client) {
        await client.disconnect();
        client = null;
      }
    },
  });

  return { stream, mimeType, totalSize, contentLength: byteCount };
}

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
  const client = await createMtprotoClient(sessionString);

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
          fileSize: data.thumbFileSize
            ? bigInt(data.thumbFileSize)
            : undefined,
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
    await client.disconnect();
  }
}
