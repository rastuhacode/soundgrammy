import { withMtprotoClient } from "./session";

export async function downloadUserProfilePhoto(
  encryptedSession: string,
): Promise<Buffer | null> {
  return withMtprotoClient(encryptedSession, async (client) => {
    const photo = await client.downloadProfilePhoto("me", { isBig: false });
    if (!photo || typeof photo === "string") return null;
    return photo.length > 0 ? photo : null;
  });
}
