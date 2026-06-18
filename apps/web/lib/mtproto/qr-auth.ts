import type { TelegramClient } from "telegram";
import { Api } from "telegram";
import { getMtprotoCredentials } from "./config";
import { createMtprotoClient, saveClientSession } from "./client";
import { saveMtprotoSession } from "../db";
import type { MtprotoUserInfo } from "./login";

export type QrAuthStatus =
  | "pending"
  | "awaiting_password"
  | "success"
  | "error";

interface QrSession {
  status: QrAuthStatus;
  qrUrl: string;
  expires: number;
  client: TelegramClient;
  user?: MtprotoUserInfo;
  error?: string;
  passwordHint?: string;
  resolvePassword?: (password: string) => void;
}

const globalStore = globalThis as typeof globalThis & {
  __soundgrammyQrSessions?: Map<string, QrSession>;
};

function getQrSessions(): Map<string, QrSession> {
  if (!globalStore.__soundgrammyQrSessions) {
    globalStore.__soundgrammyQrSessions = new Map();
  }
  return globalStore.__soundgrammyQrSessions;
}

function userToInfo(user: Api.User): MtprotoUserInfo {
  return {
    tgUserId: Number(user.id.toString()),
    firstName: user.firstName ?? "User",
    lastName: user.lastName ?? undefined,
    username: user.username ?? undefined,
  };
}

function cleanupSession(authToken: string) {
  const sessions = getQrSessions();
  const session = sessions.get(authToken);
  if (session) {
    void session.client.disconnect().catch(() => undefined);
    sessions.delete(authToken);
  }
}

export async function startQrAuth(authToken: string) {
  const existing = getQrSessions().get(authToken);
  if (existing) {
    cleanupSession(authToken);
  }

  const { apiId, apiHash } = getMtprotoCredentials();
  const client = await createMtprotoClient("");

  const session: QrSession = {
    status: "pending",
    qrUrl: "",
    expires: 0, // populated when Telegram emits the QR token
    client,
  };

  let resolveQrReady!: (value: { qrUrl: string; expires: number }) => void;
  const qrReady = new Promise<{ qrUrl: string; expires: number }>((resolve) => {
    resolveQrReady = resolve;
  });

  let qrResolved = false;

  getQrSessions().set(authToken, session);

  void (async () => {
    try {
      const user = await client.signInUserWithQrCode(
        { apiId, apiHash },
        {
          qrCode: async ({ token, expires }) => {
            const qrUrl = `tg://login?token=${token.toString("base64url")}`;
            session.qrUrl = qrUrl;
            session.expires = expires;
            if (!qrResolved) {
              qrResolved = true;
              resolveQrReady({ qrUrl, expires });
            }
          },
          password: async (hint) => {
            session.status = "awaiting_password";
            session.passwordHint = hint;
            return new Promise<string>((resolve) => {
              session.resolvePassword = resolve;
            });
          },
          onError: async (err) => {
            session.status = "error";
            session.error = err.message;
            return true;
          },
        },
      );

      if (!(user instanceof Api.User)) {
        throw new Error("Unexpected login response");
      }

      const sessionData = saveClientSession(client);
      const info = userToInfo(user);
      saveMtprotoSession(info.tgUserId, sessionData, user.phone ?? "");
      session.user = info;
      session.status = "success";
    } catch (error) {
      session.status = "error";
      session.error =
        error instanceof Error ? error.message : "QR login failed";
    } finally {
      await client.disconnect().catch(() => undefined);
    }
  })();

  const qr = await Promise.race([
    qrReady,
    new Promise<never>((_, reject) => {
      setTimeout(
        () => reject(new Error("Timed out generating QR code")),
        30_000,
      ); // 30s to receive first QR from Telegram
    }),
  ]);

  const ttlMs = Math.max(0, qr.expires * 1_000 - Date.now()); // Telegram expiry is Unix seconds
  setTimeout(() => cleanupSession(authToken), ttlMs + 5_000); // grace period after QR expiry

  return qr;
}

export function getQrAuthStatus(authToken: string) {
  const session = getQrSessions().get(authToken);
  if (!session) {
    return {
      status: "error" as const,
      error: "Session expired. Refresh the page.",
    };
  }

  return {
    status: session.status,
    error: session.error,
    passwordHint: session.passwordHint,
    user: session.user,
  };
}

export function submitQrPassword(authToken: string, password: string) {
  const session = getQrSessions().get(authToken);
  if (!session || session.status !== "awaiting_password") {
    throw new Error("No password prompt is active");
  }
  session.resolvePassword?.(password);
}

export function consumeQrSuccess(authToken: string): MtprotoUserInfo | null {
  const session = getQrSessions().get(authToken);
  if (!session || session.status !== "success" || !session.user) {
    return null;
  }
  cleanupSession(authToken);
  return session.user;
}
