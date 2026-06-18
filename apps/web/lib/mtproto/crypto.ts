import {
  createCipheriv,
  createDecipheriv,
  createHash,
  randomBytes,
} from "node:crypto";
import { getSessionSecret } from "./config";

function getKey(): Buffer {
  return createHash("sha256").update(getSessionSecret()).digest();
}

export function encryptSession(session: string): string {
  const iv = randomBytes(16); // AES-CBC block size (128-bit IV)
  const cipher = createCipheriv("aes-256-cbc", getKey(), iv); // 256-bit key derived from session secret
  const encrypted = Buffer.concat([
    cipher.update(session, "utf8"),
    cipher.final(),
  ]);
  return `${iv.toString("base64")}:${encrypted.toString("base64")}`;
}

export function decryptSession(payload: string): string {
  const [ivB64, dataB64] = payload.split(":");
  if (!ivB64 || !dataB64) {
    throw new Error("Invalid encrypted session payload");
  }
  const iv = Buffer.from(ivB64, "base64");
  const encrypted = Buffer.from(dataB64, "base64");
  const decipher = createDecipheriv("aes-256-cbc", getKey(), iv); // 256-bit key, 128-bit IV from payload
  return Buffer.concat([
    decipher.update(encrypted),
    decipher.final(),
  ]).toString("utf8");
}
