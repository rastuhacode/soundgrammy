import { Audio } from "grammy/types";

export type UserAudioPayload = Audio & {
  tg_user_id: number;
};

export async function postAudio(payload: UserAudioPayload) {
  //   return await fetch(`${process.env.BACKEND_URL}/api/v1/user-audio`, {
  //     method: "POST",
  //     body: JSON.stringify(payload),
  //   });
  return { ok: true, statusText: "ok" };
}
