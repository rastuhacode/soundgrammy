import { TelegramClient, sessions } from "telegram";
import {
  acquirePooledMtprotoClient,
  releasePooledMtprotoClient,
} from "./client-pool";
import { getMtprotoCredentials } from "./config";
import { decryptSession, encryptSession } from "./crypto";

/**
 * Telegram client lifecycle helpers: creating connected clients from a stored
 * session string and (de)serializing sessions for persistence. Sessions are
 * stored encrypted at rest; these helpers are the single place that bridges the
 * encrypted form and a live {@link TelegramClient}.
 */

/** Serializes the client's session and returns it in encrypted form for storage. */
export function saveClientSession(client: TelegramClient): string {
  return encryptSession((client.session as sessions.StringSession).save());
}

/**
 * Creates and connects a Telegram client from a (decrypted) session string.
 * Pass an empty string to start an anonymous client, e.g. for login flows.
 */
export async function createMtprotoClient(
  sessionString = "",
): Promise<TelegramClient> {
  const { apiId, apiHash } = getMtprotoCredentials();
  const client = new TelegramClient(
    new sessions.StringSession(sessionString),
    apiId,
    apiHash,
    { connectionRetries: 3 }, // retry transient DC connection failures
  );
  await client.connect();
  return client;
}

/** Stops the background update loop and tears down a short-lived client. */
export async function destroyMtprotoClient(
  client: TelegramClient,
): Promise<void> {
  await client.destroy().catch(() => undefined);
}

/**
 * Runs `fn` with a pooled client built from an encrypted session, releasing it
 * afterwards so later requests reuse the same MTProto connection. Use this for
 * short request/response interactions; for long-lived streaming, acquire and
 * release from the pool manually around the stream lifecycle.
 */
export async function withMtprotoClient<T>(
  encryptedSession: string,
  fn: (client: TelegramClient) => Promise<T>,
): Promise<T> {
  const sessionString = decryptSession(encryptedSession);
  const client = await acquirePooledMtprotoClient(sessionString);
  try {
    return await fn(client);
  } finally {
    releasePooledMtprotoClient(sessionString);
  }
}
