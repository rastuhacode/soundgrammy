import bigInt, { type BigInteger } from "big-integer";
import type { TelegramClient } from "teleproto";
import { Api } from "teleproto";

/** Telegram saved-music page size (max documents returned per request). */
export const SAVED_MUSIC_PAGE_SIZE = 100;

export type SavedMusicResult = Api.users.TypeSavedMusic;

interface GetSavedMusicParams {
  offset: number;
  limit: number;
  /** "0" forces a fetch; a non-zero hash lets the server short-circuit when unchanged. */
  hash?: string | BigInteger;
}

/** Fetches a page of the current user's profile (saved) music. */
export function getSavedMusic(
  client: TelegramClient,
  params: GetSavedMusicParams,
): Promise<SavedMusicResult> {
  const hash = params.hash ?? "0";
  const hashInt = typeof hash === "string" ? bigInt(hash) : hash;

  return client.invoke(
    new Api.users.GetSavedMusic({
      id: new Api.InputUserSelf(),
      offset: params.offset,
      limit: params.limit,
      hash: hashInt,
    }),
  );
}

/** Re-fetches specific saved-music documents by their input document refs. */
export function getSavedMusicByID(
  client: TelegramClient,
  documents: Api.TypeInputDocument[],
): Promise<SavedMusicResult> {
  return client.invoke(
    new Api.users.GetSavedMusicByID({
      id: new Api.InputUserSelf(),
      documents,
    }),
  );
}
