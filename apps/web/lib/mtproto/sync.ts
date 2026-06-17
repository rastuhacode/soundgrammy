import { Api } from "telegram";
import type { TelegramClient } from "telegram";
import { insertTrack, type TrackSource } from "../db";
import {
  GetSavedMusicRequest,
  registerSavedMusicTl,
  type SavedMusicResult,
} from "./saved-music-tl";

registerSavedMusicTl();

function parseDocument(doc: Api.Document): {
  title: string | null;
  performer: string | null;
  duration: number | null;
  storedJson: string;
} {
  let title: string | null = null;
  let performer: string | null = null;
  let duration: number | null = null;

  for (const attr of doc.attributes ?? []) {
    if (attr instanceof Api.DocumentAttributeAudio) {
      title = attr.title ?? null;
      performer = attr.performer ?? null;
      duration = attr.duration ?? null;
    }
    if (attr instanceof Api.DocumentAttributeFilename && !title) {
      title = attr.fileName;
    }
  }

  const storedJson = JSON.stringify({
    id: doc.id.toString(),
    accessHash: doc.accessHash?.toString() ?? "0",
    fileReference: Buffer.from(doc.fileReference).toString("base64"),
    dcId: doc.dcId,
    mimeType: doc.mimeType ?? "audio/mpeg",
    size: doc.size?.toString() ?? "0",
  });

  return { title, performer, duration, storedJson };
}

async function fetchAllProfileMusic(
  client: TelegramClient,
): Promise<Api.Document[]> {
  const documents: Api.Document[] = [];
  let offset = 0;
  const limit = 100;

  while (true) {
    const result = (await client.invoke(
      new GetSavedMusicRequest({ offset, limit }) as never,
    )) as SavedMusicResult;

    if (result.className === "users.savedMusicNotModified") {
      break;
    }

    documents.push(...result.documents);
    if (result.documents.length < limit) {
      break;
    }
    offset += result.documents.length;
  }

  return documents;
}

export async function syncProfileMusic(
  client: TelegramClient,
  tgUserId: number,
): Promise<{ imported: number; total: number }> {
  const documents = await fetchAllProfileMusic(client);
  let imported = 0;

  for (const doc of documents) {
    const { title, performer, duration, storedJson } = parseDocument(doc);
    const fileUniqueId = `mtproto:${doc.id.toString()}`;

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

  return { imported, total: documents.length };
}
