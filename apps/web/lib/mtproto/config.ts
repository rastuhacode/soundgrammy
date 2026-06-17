export function getMtprotoCredentials() {
  const apiId = Number(process.env.TELEGRAM_API_ID);
  const apiHash = process.env.TELEGRAM_API_HASH;

  if (!apiId || !apiHash) {
    throw new Error(
      "TELEGRAM_API_ID and TELEGRAM_API_HASH must be set (get them from my.telegram.org)",
    );
  }

  return { apiId, apiHash };
}

export function getSessionSecret(): string {
  const secret =
    process.env.MTPROTO_SESSION_SECRET ?? process.env.JWT_SECRET ?? "";
  if (!secret) {
    throw new Error("MTPROTO_SESSION_SECRET or JWT_SECRET must be set");
  }
  return secret;
}
