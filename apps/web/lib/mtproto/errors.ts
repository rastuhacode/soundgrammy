/**
 * Helpers for interpreting errors thrown by the Telegram MTProto client.
 *
 * teleproto surfaces RPC errors in two shapes depending on the call path: some
 * carry a structured `errorMessage` property, while others only embed the code
 * in the standard `Error.message`. These helpers normalize both so callers can
 * react to specific server errors without re-implementing the inspection logic.
 */

/** Telegram error returned when a stored file reference is stale and must be refetched. */
const FILE_REFERENCE_ERRORS = [
  "FILE_REFERENCE_EXPIRED",
  "FILE_REFERENCE_INVALID",
] as const;

/** Telegram error returned when a 2FA password is required to finish sign-in. */
const SESSION_PASSWORD_NEEDED = "SESSION_PASSWORD_NEEDED";

/**
 * Extracts the Telegram error code from an unknown thrown value, checking both
 * the structured `errorMessage` field and the plain `Error.message`.
 *
 * @returns The raw error string, or `null` if the value is not an error.
 */
export function getTelegramErrorMessage(error: unknown): string | null {
  if (error && typeof error === "object" && "errorMessage" in error) {
    return String((error as { errorMessage: unknown }).errorMessage);
  }
  if (error instanceof Error) {
    return error.message;
  }
  return null;
}

/** True when the error means a cached file reference expired and must be refreshed. */
export function isFileReferenceError(error: unknown): boolean {
  const message = getTelegramErrorMessage(error);
  if (!message) {
    return false;
  }
  return FILE_REFERENCE_ERRORS.some((code) => message.includes(code));
}

/** True when Telegram is asking for the account's 2FA password to continue. */
export function isSessionPasswordNeeded(error: unknown): boolean {
  return getTelegramErrorMessage(error) === SESSION_PASSWORD_NEEDED;
}
