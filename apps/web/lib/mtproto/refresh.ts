import type { TelegramClient } from "telegram";
import {
  documentToStoredJson,
  extractThumbFromDocument,
  mergeStoredDocumentThumb,
  shouldUpgradeStoredThumb,
  toInputDocument,
} from "./document";
import { parseStoredDocument, type StoredDocument } from "./document";
import {
  GetSavedMusicByIDRequest,
  GetSavedMusicRequest,
  registerSavedMusicTl,
  type SavedMusicResult,
} from "./saved-music-tl";

registerSavedMusicTl();

async function findSavedMusicDocument(
  client: TelegramClient,
  stored: StoredDocument,
): Promise<import("telegram").Api.Document | undefined> {
  let offset = 0;
  const limit = 100;

  while (true) {
    const result = (await client.invoke(
      new GetSavedMusicRequest({ offset, limit, hash: "0" }) as never,
    )) as SavedMusicResult;

    if (result.className === "users.savedMusicNotModified") {
      return undefined;
    }

    const found = result.documents.find(
      (doc) => doc.id.toString() === stored.id,
    );
    if (found) {
      return found;
    }

    if (result.documents.length < limit) {
      return undefined;
    }
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
  const result = (await client.invoke(
    new GetSavedMusicByIDRequest([toInputDocument(stored)]) as never,
  )) as SavedMusicResult;

  if (result.className === "users.savedMusicNotModified") {
    throw new Error("Saved music document is no longer on profile");
  }

  const refreshed = result.documents.find(
    (doc) => doc.id.toString() === stored.id,
  );
  if (!refreshed) {
    throw new Error("Saved music document is no longer on profile");
  }

  const next = parseStoredDocument(documentToStoredJson(refreshed));
  return attachThumbMetadata(client, stored, next);
}

function applyThumbMetadata(
  stored: StoredDocument,
  doc: import("telegram").Api.Document,
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
  if (!shouldUpgradeStoredThumb(stored)) {
    return stored;
  }

  const fromList = await findSavedMusicDocument(client, stored);
  if (!fromList) {
    return stored;
  }

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
