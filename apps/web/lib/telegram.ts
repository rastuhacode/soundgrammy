import { createHmac, createHash } from "node:crypto";

export interface TelegramLoginData {
  id: number;
  first_name: string;
  last_name?: string;
  username?: string;
  photo_url?: string;
  auth_date: number;
  hash: string;
}

const MAX_AUTH_AGE_SECONDS = 86400; // 24 hours

export function verifyTelegramLogin(data: TelegramLoginData): boolean {
  const botToken = process.env.BOT_TOKEN;
  if (!botToken) {
    throw new Error("BOT_TOKEN is not set");
  }

  // Check auth_date is not too old
  const now = Math.floor(Date.now() / 1000);
  if (now - data.auth_date > MAX_AUTH_AGE_SECONDS) {
    return false;
  }

  // Build data-check-string: sort all fields except hash, join as key=value with \n
  const { hash, ...rest } = data;
  const dataCheckString = Object.keys(rest)
    .sort()
    .map((key) => `${key}=${rest[key as keyof typeof rest]}`)
    .join("\n");

  // secret_key = SHA256(BOT_TOKEN)
  const secretKey = createHash("sha256").update(botToken).digest();

  // computed_hash = HMAC-SHA256(data_check_string, secret_key)
  const computedHash = createHmac("sha256", secretKey)
    .update(dataCheckString)
    .digest("hex");

  return computedHash === hash;
}
