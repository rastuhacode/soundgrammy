import type { TelegramClient } from "telegram";
import {
  deleteMtprotoTracksNotIn,
  getMtprotoSession,
  getTracksByUser,
  insertTrack,
  updateMtprotoLastSync,
  updateMtprotoSavedMusicHash,
  type TrackSource,
} from "../db";
import { withMtprotoClient } from "./client";
import {
  computeSavedMusicHash,
  parseDocumentMetadata,
} from "./document";
import {
  GetSavedMusicRequest,
  registerSavedMusicTl,
  type SavedMusicResult,
} from "./saved-music-tl";

registerSavedMusicTl();

export interface SyncProfileMusicOptions {
  storedHash?: string | null;
}

export interface SyncProfileMusicResult {
  imported: number;
  removed: number;
  total: number;
  notModified: boolean;
  hash: string;
}

async function fetchAllProfileMusic(
  client: TelegramClient,
  hash: string,
): Promise<
  | { notModified: true; count: number }
  | { notModified: false; documents: import("telegram").Api.Document[] }
> {
  const documents: import("telegram").Api.Document[] = [];
  let offset = 0;
  const limit = 100;

  while (true) {
    const result = (await client.invoke(
      new GetSavedMusicRequest({ offset, limit, hash }) as never,
    )) as SavedMusicResult;

    if (result.className === "users.savedMusicNotModified") {
      return { notModified: true, count: result.count };
    }

    documents.push(...result.documents);
    if (result.documents.length < limit) {
      break;
    }
    offset += result.documents.length;
  }

  return { notModified: false, documents };
}

export async function syncProfileMusic(
  client: TelegramClient,
  tgUserId: number,
  options: SyncProfileMusicOptions = {},
): Promise<SyncProfileMusicResult> {
  const localTracks = getTracksByUser(tgUserId);
  const hash =
    localTracks.length === 0 ? "0" : (options.storedHash ?? "0");
  const fetched = await fetchAllProfileMusic(client, hash);

  if (fetched.notModified) {
    return {
      imported: 0,
      removed: 0,
      total: fetched.count,
      notModified: true,
      hash,
    };
  }

  const documents = fetched.documents;
  const fileUniqueIds: string[] = [];
  let imported = 0;

  for (const doc of documents) {
    const { title, performer, duration, storedJson } =
      parseDocumentMetadata(doc);
    const fileUniqueId = `mtproto:${doc.id.toString()}`;
    fileUniqueIds.push(fileUniqueId);

    insertTrack({
      tg_user_id: tgUserId,
      file_id: fileUniqueId,
      file_unique_id: fileUniqueId,
      title: title ?? undefined,
      performer: performer ?? undefined,
      duration: duration ?? undefined,
      source: "mtproto" satisfies TrackSource,
      mime_type: doc.mimeType ?? "audio/mpeg",
      mtproto_document: storedJson,
      file_name: title ?? `track-${doc.id.toString()}`,
      file_size: Number(doc.size?.toString() ?? 0),
    });
    imported++;
  }

  const removed = deleteMtprotoTracksNotIn(tgUserId, fileUniqueIds);
  const nextHash = computeSavedMusicHash(
    documents.map((doc) => doc.id.toString()),
  );

  return {
    imported,
    removed,
    total: documents.length,
    notModified: false,
    hash: nextHash,
  };
}

export async function ensureProfileMusicSynced(tgUserId: number): Promise<void> {
  if (getTracksByUser(tgUserId).length > 0) {
    return;
  }

  const mtprotoSession = getMtprotoSession(tgUserId);
  if (!mtprotoSession) {
    return;
  }

  const result = await withMtprotoClient(
    mtprotoSession.session_data,
    async (client) =>
      syncProfileMusic(client, tgUserId, {
        storedHash: mtprotoSession.saved_music_hash,
      }),
  );

  updateMtprotoLastSync(tgUserId);
  if (!result.notModified) {
    updateMtprotoSavedMusicHash(tgUserId, result.hash);
  }
}
