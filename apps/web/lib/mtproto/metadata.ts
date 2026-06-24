import { Api } from "teleproto";
import {
  isDocument,
  parseStoredDocument,
  serializeDocumentAttributes,
  toInputDocument,
  type SerializedDocumentAttribute,
} from "./document";
import { getSavedMusicByID } from "./saved-music-tl";
import { withMtprotoClient } from "./session";

/** Resolved Telegram document fields used by track info and downloads. */
export interface TrackDocumentMetadata {
  documentId: string;
  dcId: number;
  mimeType: string | null;
  size: string | null;
  attributes: SerializedDocumentAttribute[];
  source: "stored" | "telegram";
}

/** Re-fetches one saved-music document from Telegram by its stored ids. */
async function fetchDocumentFromTelegram(
  client: Parameters<typeof getSavedMusicByID>[0],
  storedJson: string,
): Promise<Api.Document | null> {
  const stored = parseStoredDocument(storedJson);
  const result = await getSavedMusicByID(client, [toInputDocument(stored)]);

  if (result instanceof Api.users.SavedMusicNotModified) {
    return null;
  }

  return (
    result.documents.find(
      (doc): doc is Api.Document =>
        isDocument(doc) && doc.id.toString() === stored.id,
    ) ?? null
  );
}

/**
 * Returns document attributes and size/mime metadata, using the stored JSON when
 * attributes are already cached and otherwise fetching from Telegram once.
 */
export async function resolveTrackDocumentMetadata(
  encryptedSession: string,
  storedJson: string,
): Promise<TrackDocumentMetadata> {
  const stored = parseStoredDocument(storedJson);

  if (stored.attributes?.length) {
    return {
      documentId: stored.id,
      dcId: stored.dcId,
      mimeType: stored.mimeType ?? null,
      size: stored.size ?? null,
      attributes: stored.attributes,
      source: "stored",
    };
  }

  return withMtprotoClient(encryptedSession, async (client) => {
    const doc = await fetchDocumentFromTelegram(client, storedJson);
    if (!doc) {
      return {
        documentId: stored.id,
        dcId: stored.dcId,
        mimeType: stored.mimeType ?? null,
        size: stored.size ?? null,
        attributes: [],
        source: "stored",
      };
    }

    return {
      documentId: doc.id.toString(),
      dcId: doc.dcId,
      mimeType: doc.mimeType ?? stored.mimeType ?? null,
      size: doc.size?.toString() ?? stored.size ?? null,
      attributes: serializeDocumentAttributes(doc.attributes),
      source: "telegram",
    };
  });
}
