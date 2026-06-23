import { Api, type TelegramClient } from "teleproto";
import {
  documentToStoredJson,
  extractThumbFromDocument,
  isDocument,
  mergeStoredDocumentThumb,
  shouldUpgradeStoredThumb,
  toInputDocument,
} from "./document";
import { parseStoredDocument, type StoredDocument } from "./document";
import {
  getSavedMusic,
  getSavedMusicByID,
  SAVED_MUSIC_PAGE_SIZE,
} from "./saved-music-tl";

async function findSavedMusicDocument(
  client: TelegramClient,
  stored: StoredDocument,
): Promise<Api.Document | undefined> {
  let offset = 0;

  while (true) {
    const result = await getSavedMusic(client, {
      offset,
      limit: SAVED_MUSIC_PAGE_SIZE,
      hash: "0", // "0" = no client-side cache hash
    });

    if (result instanceof Api.users.SavedMusicNotModified) return;

    const found = result.documents.find(
      (doc): doc is Api.Document =>
        isDocument(doc) && doc.id.toString() === stored.id,
    );
    if (found) return found;

    if (result.documents.length < SAVED_MUSIC_PAGE_SIZE) return;
    offset += result.documents.length;
  }
}

async function attachThumbMetadata(
  client: TelegramClient,
  stored: StoredDocument,
  refreshed: StoredDocument,
): Promise<StoredDocument> {
  if (refreshed.thumbSize || refreshed.thumbData) {
    return refreshed;
  }

  const fromList = await findSavedMusicDocument(client, stored);
  if (fromList) {
    return applyThumbMetadata(refreshed, fromList);
  }

  return mergeStoredDocumentThumb(refreshed, stored);
}

export async function refreshSavedMusicDocument(
  client: TelegramClient,
  stored: StoredDocument,
): Promise<StoredDocument> {
  const result = await getSavedMusicByID(client, [toInputDocument(stored)]);

  if (result instanceof Api.users.SavedMusicNotModified) {
    throw new Error("Saved music document is no longer on profile");
  }

  const refreshed = result.documents.find(
    (doc): doc is Api.Document =>
      isDocument(doc) && doc.id.toString() === stored.id,
  );
  if (!refreshed) {
    throw new Error("Saved music document is no longer on profile");
  }

  const next = parseStoredDocument(documentToStoredJson(refreshed));
  return attachThumbMetadata(client, stored, next);
}

function applyThumbMetadata(
  stored: StoredDocument,
  doc: Api.Document,
): StoredDocument {
  const thumb = extractThumbFromDocument(doc);
  return {
    ...stored,
    thumbSize: thumb.thumbSize,
    thumbFileSize: thumb.thumbFileSize,
    thumbData: thumb.thumbSize ? undefined : thumb.thumbData,
  };
}

export async function enrichSavedMusicDocumentThumb(
  client: TelegramClient,
  stored: StoredDocument,
): Promise<StoredDocument> {
  if (!shouldUpgradeStoredThumb(stored)) return stored;

  const fromList = await findSavedMusicDocument(client, stored);
  if (!fromList) return stored;

  return applyThumbMetadata(stored, fromList);
}

export async function refreshSavedMusicDocumentJson(
  client: TelegramClient,
  storedJson: string,
): Promise<string> {
  const refreshed = await refreshSavedMusicDocument(
    client,
    parseStoredDocument(storedJson),
  );
  return JSON.stringify(refreshed);
}
