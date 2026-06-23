import type { TelegramClient } from "teleproto";
import { createMtprotoClient } from "./session";

/**
 * Reuses one connected {@link TelegramClient} per decrypted session string so
 * successive HTTP range windows (and concurrent streams) share a single MTProto
 * connection instead of reconnecting on every request.
 */

interface PoolEntry {
  client?: TelegramClient;
  connectPromise?: Promise<TelegramClient>;
  refCount: number;
}

const pool = new Map<string, PoolEntry>();

async function connectEntry(
  sessionString: string,
  entry: PoolEntry,
): Promise<TelegramClient> {
  if (entry.client) {
    if (!entry.client.connected) {
      await entry.client.connect();
    }
    return entry.client;
  }

  if (entry.connectPromise) {
    return entry.connectPromise;
  }

  entry.connectPromise = createMtprotoClient(sessionString)
    .then((client) => {
      entry.client = client;
      entry.connectPromise = undefined;
      return client;
    })
    .catch((error) => {
      entry.connectPromise = undefined;
      throw error;
    });

  return entry.connectPromise;
}

/** Returns a pooled client for `sessionString`, creating or reconnecting as needed. */
export async function acquirePooledMtprotoClient(
  sessionString: string,
): Promise<TelegramClient> {
  let entry = pool.get(sessionString);
  if (!entry) {
    entry = { refCount: 0 };
    pool.set(sessionString, entry);
  }

  entry.refCount++;
  try {
    return await connectEntry(sessionString, entry);
  } catch (error) {
    entry.refCount = Math.max(0, entry.refCount - 1);
    if (entry.refCount === 0 && !entry.client) {
      pool.delete(sessionString);
    }
    throw error;
  }
}

/**
 * Releases a pooled client acquired via {@link acquirePooledMtprotoClient}.
 * The connection stays open for reuse by later range windows.
 */
export function releasePooledMtprotoClient(sessionString: string): void {
  const entry = pool.get(sessionString);
  if (!entry) {
    return;
  }

  entry.refCount = Math.max(0, entry.refCount - 1);
}
