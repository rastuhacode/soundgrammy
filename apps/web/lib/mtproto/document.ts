import bigInt from "big-integer";
import { Api } from "telegram";
import { strippedPhotoToJpg } from "telegram/Utils";

export interface StoredDocument {
  id: string;
  accessHash: string;
  fileReference: string;
  dcId: number;
  mimeType?: string;
  size?: string;
  thumbSize?: string;
  thumbFileSize?: string;
  thumbData?: string;
}

export function parseStoredDocument(storedJson: string): StoredDocument {
  return JSON.parse(storedJson) as StoredDocument;
}

export function isFileReferenceError(error: unknown): boolean {
  if (error && typeof error === "object" && "errorMessage" in error) {
    const message = String((error as { errorMessage: string }).errorMessage);
    return (
      message === "FILE_REFERENCE_EXPIRED" ||
      message === "FILE_REFERENCE_INVALID"
    );
  }
  if (error instanceof Error) {
    return (
      error.message.includes("FILE_REFERENCE_EXPIRED") ||
      error.message.includes("FILE_REFERENCE_INVALID")
    );
  }
  return false;
}

function thumbByteCount(thumb: Api.TypePhotoSize): number {
  if (thumb instanceof Api.PhotoStrippedSize) return thumb.bytes.length;
  if (thumb instanceof Api.PhotoCachedSize) return thumb.bytes.length;
  if (thumb instanceof Api.PhotoSize) return thumb.size;
  if (thumb instanceof Api.PhotoSizeProgressive) {
    return Math.max(...thumb.sizes);
  }
  return 0;
}

function photoWidth(thumb: Api.TypePhotoSize): number {
  if (thumb instanceof Api.PhotoSize) return thumb.w;
  if (thumb instanceof Api.PhotoSizeProgressive) return thumb.w;
  return 0;
}

/** Prefer ~320px "m" thumbs: sharp on retina, still a small download. */
const THUMB_TARGET_WIDTH = 320;
const THUMB_MAX_DOWNLOAD_BYTES = 80_000;

function pickBestRemoteThumb(
  thumbs: Api.TypePhotoSize[],
): Api.PhotoSize | Api.PhotoSizeProgressive | undefined {
  const remote = thumbs.filter(
    (thumb) =>
      thumb instanceof Api.PhotoSize ||
      thumb instanceof Api.PhotoSizeProgressive,
  );
  if (remote.length === 0) {
    return undefined;
  }

  const medium = remote.find(
    (thumb) => thumb instanceof Api.PhotoSize && thumb.type === "m",
  );
  if (medium) {
    return medium;
  }

  const eligible = remote
    .filter((thumb) => {
      const bytes = thumbByteCount(thumb);
      return bytes > 0 && bytes <= THUMB_MAX_DOWNLOAD_BYTES;
    })
    .sort((a, b) => {
      const distA = Math.abs(photoWidth(a) - THUMB_TARGET_WIDTH);
      const distB = Math.abs(photoWidth(b) - THUMB_TARGET_WIDTH);
      if (distA !== distB) {
        return distA - distB;
      }
      return thumbByteCount(a) - thumbByteCount(b);
    });

  return eligible[0];
}

export function isLowQualityInlineThumb(thumbData: string): boolean {
  return Buffer.from(thumbData, "base64").length < 8_000;
}

export function shouldUpgradeStoredThumb(stored: StoredDocument): boolean {
  if (stored.thumbSize) {
    return false;
  }
  if (!stored.thumbData) {
    return true;
  }
  return isLowQualityInlineThumb(stored.thumbData);
}

export function extractThumbFromDocument(doc: Api.Document): {
  thumbSize?: string;
  thumbFileSize?: string;
  thumbData?: string;
} {
  const thumbs = (doc.thumbs ?? []).filter(
    (thumb) => !(thumb instanceof Api.PhotoPathSize),
  );
  if (thumbs.length === 0) {
    return {};
  }

  const remote = pickBestRemoteThumb(thumbs);
  if (remote instanceof Api.PhotoSize) {
    return {
      thumbSize: remote.type,
      thumbFileSize: String(remote.size),
    };
  }
  if (remote instanceof Api.PhotoSizeProgressive) {
    return {
      thumbSize: remote.type,
      thumbFileSize: String(Math.max(...remote.sizes)),
    };
  }

  for (const thumb of thumbs) {
    if (thumb instanceof Api.PhotoCachedSize) {
      return { thumbData: Buffer.from(thumb.bytes).toString("base64") };
    }
  }

  for (const thumb of thumbs) {
    if (thumb instanceof Api.PhotoStrippedSize) {
      return {
        thumbData: Buffer.from(strippedPhotoToJpg(thumb.bytes)).toString(
          "base64",
        ),
      };
    }
  }

  return {};
}

export function mergeStoredDocumentThumb(
  refreshed: StoredDocument,
  previous?: StoredDocument,
): StoredDocument {
  if (refreshed.thumbSize || refreshed.thumbData) {
    return refreshed;
  }
  if (!previous?.thumbSize && !previous?.thumbData) {
    return refreshed;
  }
  return {
    ...refreshed,
    thumbSize: previous.thumbSize,
    thumbFileSize: previous.thumbFileSize,
    thumbData: previous.thumbData,
  };
}

export function documentToStoredJson(doc: Api.Document): string {
  const thumb = extractThumbFromDocument(doc);
  return JSON.stringify({
    id: doc.id.toString(),
    accessHash: doc.accessHash?.toString() ?? "0",
    fileReference: Buffer.from(doc.fileReference).toString("base64"),
    dcId: doc.dcId,
    mimeType: doc.mimeType ?? "audio/mpeg",
    size: doc.size?.toString() ?? "0",
    ...thumb,
  } satisfies StoredDocument);
}

export function parseDocumentMetadata(doc: Api.Document): {
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

  return {
    title,
    performer,
    duration,
    storedJson: documentToStoredJson(doc),
  };
}

export function computeSavedMusicHash(documentIds: string[]): string {
  let hash = bigInt.zero;
  for (const id of documentIds) {
    hash = hash.xor(bigInt(id));
  }
  return hash.toString();
}

export function toInputDocument(data: StoredDocument): Api.InputDocument {
  return new Api.InputDocument({
    id: bigInt(data.id),
    accessHash: bigInt(data.accessHash),
    fileReference: Buffer.alloc(0),
  });
}
