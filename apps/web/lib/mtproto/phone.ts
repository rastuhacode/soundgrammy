export function normalizePhoneNumber(phone: string): string {
  let cleaned = phone.replace(/[\s()-]/g, "");
  if (cleaned.startsWith("8") && cleaned.length === 11) {
    cleaned = `+7${cleaned.slice(1)}`;
  }
  if (!cleaned.startsWith("+")) {
    throw new Error(
      "Phone number must include country code, for example +79991234567",
    );
  }
  if (!/^\+\d{7,15}$/.test(cleaned)) {
    throw new Error("Phone number format is invalid");
  }
  return cleaned;
}

export type CodeDelivery = "app" | "sms";

export function getCodeDelivery(isCodeViaApp: boolean): CodeDelivery {
  return isCodeViaApp ? "app" : "sms";
}
