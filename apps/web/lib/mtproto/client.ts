import { TelegramClient, sessions, Api } from "telegram";
import bigInt from "big-integer";
import { getMtprotoCredentials } from "./config";
import { decryptSession, encryptSession } from "./crypto";

const TELEGRAM_REQUEST_SIZE = 256 * 1024;

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

export interface StoredDocument {
  id: string;
  accessHash: string;
  fileReference: string;
  dcId: number;
  mimeType?: string;
  size?: string;
}

export function parseStoredDocument(storedJson: string): StoredDocument {
  return JSON.parse(storedJson) as StoredDocument;
}

function buildDocumentLocation(
  data: StoredDocument,
): Api.InputDocumentFileLocation {
  return new Api.InputDocumentFileLocation({
    id: bigInt(data.id),
    accessHash: bigInt(data.accessHash),
    fileReference: Buffer.from(data.fileReference, "base64"),
    thumbSize: "",
  });
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
): MtprotoDocumentStream {
  const data = parseStoredDocument(storedJson);
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
            controller.enqueue(new Uint8Array(buffer.subarray(0, remaining)));
            bytesSent += remaining;
            break;
          }

          controller.enqueue(new Uint8Array(buffer));
          bytesSent += buffer.length;
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
