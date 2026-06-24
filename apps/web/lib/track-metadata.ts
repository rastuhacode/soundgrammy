import { getMtprotoSession, getTrackById } from "@/lib/db";
import { parseStoredDocument } from "@/lib/mtproto/client";
import { resolveTrackDocumentMetadata } from "@/lib/mtproto/metadata";
import type { SerializedDocumentAttribute } from "@/lib/mtproto/document";

/** Full track info payload returned by the `tracks.metadata` tRPC procedure. */
export interface TrackMetadata {
  track: {
    id: number;
    title: string | null;
    performer: string | null;
    duration: number | null;
    mimeType: string | null;
    fileSize: number | null;
    fileId: string;
    fileUniqueId: string;
    source: string;
    createdAt: string;
  };
  document: {
    id: string;
    dcId: number;
    mimeType: string | null;
    size: string | null;
    hasInlineThumb: boolean;
    hasRemoteThumb: boolean;
    attributesSource: "stored" | "telegram";
    attributes: SerializedDocumentAttribute[];
  };
  thumbnailUrl: string;
}

/**
 * Loads library fields plus Telegram document metadata for the track info
 * dialog. Requires an active MTProto session and a stored document reference.
 */
export async function getTrackMetadata(
  trackId: number,
  tgUserId: number,
): Promise<TrackMetadata> {
  const track = getTrackById(trackId, tgUserId);
  if (!track) {
    throw new Error("Track not found");
  }

  const mtprotoSession = getMtprotoSession(tgUserId);
  if (!mtprotoSession || !track.mtproto_document) {
    throw new Error("MTProto session required for this track");
  }

  const storedDocument = parseStoredDocument(track.mtproto_document);
  const documentMetadata = await resolveTrackDocumentMetadata(
    mtprotoSession.session_data,
    track.mtproto_document,
  );

  return {
    track: {
      id: track.id,
      title: track.title,
      performer: track.performer,
      duration: track.duration,
      mimeType: track.mime_type,
      fileSize: track.file_size,
      fileId: track.file_id,
      fileUniqueId: track.file_unique_id,
      source: track.source,
      createdAt: track.created_at,
    },
    document: {
      id: storedDocument.id,
      dcId: storedDocument.dcId,
      mimeType: documentMetadata.mimeType ?? storedDocument.mimeType ?? null,
      size: documentMetadata.size ?? storedDocument.size ?? null,
      hasInlineThumb: Boolean(storedDocument.thumbData),
      hasRemoteThumb: Boolean(storedDocument.thumbSize),
      attributesSource: documentMetadata.source,
      attributes: documentMetadata.attributes,
    },
    thumbnailUrl: `/api/tracks/${track.id}/thumbnail`,
  };
}
