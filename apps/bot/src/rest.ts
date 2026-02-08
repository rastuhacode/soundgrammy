import { Audio } from "grammy/types";

export type UserAudioPayload = Audio & {
  tg_user_id: number;
};

export async function postAudio(payload: UserAudioPayload) {
  const backendUrl = process.env.BACKEND_URL;
  if (!backendUrl) {
    throw new Error("BACKEND_URL is not set");
  }

  const apiKey = process.env.BOT_API_KEY;
  if (!apiKey) {
    throw new Error("BOT_API_KEY is not set");
  }

  return await fetch(`${backendUrl}/api/tracks`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-api-key": apiKey,
    },
    body: JSON.stringify(payload),
  });
}
