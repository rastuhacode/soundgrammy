// TODO: replace with frontend phone zod validdation + transform
export function normalizePhoneNumber(phone: string): string {
  let cleaned = phone.replace(/[\s()-]/g, "");
  // Russian local format: 8XXXXXXXXXX (11 digits) → +7XXXXXXXXXX
  if (cleaned.startsWith("8") && cleaned.length === 11) {
    cleaned = `+7${cleaned.slice(1)}`; // replace leading 8 with country code +7
  }
  if (!cleaned.startsWith("+")) {
    throw new Error(
      "Phone number must include country code, for example +79991234567",
    );
  }
  if (!/^\+\d{7,15}$/.test(cleaned)) {
    // E.164: country code + 7–15 subscriber digits
    throw new Error("Phone number format is invalid");
  }
  return cleaned;
}

export type CodeDelivery = "app" | "sms";

export function getCodeDelivery(isCodeViaApp: boolean): CodeDelivery {
  return isCodeViaApp ? "app" : "sms";
}
