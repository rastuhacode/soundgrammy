import { NextResponse } from "next/server";
import { getSession } from "@/lib/auth";
import { extractEmbeddedCover, readStreamPrefix } from "@/lib/audio/cover";
import {
  getMtprotoSession,
  getTrackById,
  updateTrackMtprotoDocument,
} from "@/lib/db";
import {
  createMtprotoDocumentStream,
  createMtprotoThumbnailStream,
  withMtprotoClient,
} from "@/lib/mtproto/client";
import { enrichSavedMusicDocumentThumb } from "@/lib/mtproto/refresh";
import {
  parseStoredDocument,
  shouldUpgradeStoredThumb,
} from "@/lib/mtproto/document";

function coverContentType(format: string): string {
  if (format.includes("png")) return "image/png";
  if (format.includes("webp")) return "image/webp";
  return "image/jpeg";
}

function thumbnailResponseHeaders(
  contentType: string,
  contentLength?: number,
): Record<string, string> {
  const headers: Record<string, string> = {
    "Content-Type": contentType,
    "Cache-Control": "private, max-age=86400",
  };
  if (contentLength !== undefined && contentLength > 0) {
    headers["Content-Length"] = String(contentLength);
  }
  return headers;
}

function bufferToStream(buffer: Buffer): ReadableStream<Uint8Array> {
  return new ReadableStream({
    start(controller) {
      controller.enqueue(new Uint8Array(buffer));
      controller.close();
    },
  });
}

export async function GET(
  _request: Request,
  { params }: { params: Promise<{ trackId: string }> },
) {
  const session = await getSession();

  if (!session) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  const { trackId } = await params;
  const id = Number(trackId);
  if (!Number.isFinite(id)) {
    return NextResponse.json({ error: "Invalid track id" }, { status: 400 });
  }

  const track = getTrackById(id, session.tgUserId);
  if (!track) {
    return NextResponse.json({ error: "Track not found" }, { status: 404 });
  }

  const mtprotoSession = getMtprotoSession(session.tgUserId);
  if (!mtprotoSession || !track.mtproto_document) {
    return NextResponse.json(
      { error: "MTProto session required for this track" },
      { status: 400 },
    );
  }

  let storedJson = track.mtproto_document;
  let stored = parseStoredDocument(storedJson);

  if (shouldUpgradeStoredThumb(stored)) {
    try {
      const enriched = await withMtprotoClient(
        mtprotoSession.session_data,
        async (client) =>
          enrichSavedMusicDocumentThumb(
            client,
            parseStoredDocument(storedJson),
          ),
      );
      storedJson = JSON.stringify(enriched);
      stored = enriched;
      updateTrackMtprotoDocument(track.id, session.tgUserId, storedJson);
    } catch {
      if (!stored.thumbSize && !stored.thumbData) {
        return NextResponse.json({ error: "No thumbnail" }, { status: 404 });
      }
    }
  }

  const onDocumentRefreshed = (refreshedDocument: string) => {
    updateTrackMtprotoDocument(track.id, session.tgUserId, refreshedDocument);
  };

  try {
    if (stored.thumbSize) {
      const thumbnailStream = createMtprotoThumbnailStream(
        mtprotoSession.session_data,
        storedJson,
        onDocumentRefreshed,
      );

      if (thumbnailStream) {
        return new Response(thumbnailStream.stream, {
          status: 200,
          headers: thumbnailResponseHeaders(
            "image/jpeg",
            thumbnailStream.contentLength,
          ),
        });
      }
    }

    if (stored.thumbData) {
      const thumbnail = Buffer.from(stored.thumbData, "base64");
      return new Response(bufferToStream(thumbnail), {
        status: 200,
        headers: thumbnailResponseHeaders("image/jpeg", thumbnail.length),
      });
    }

    const { stream, mimeType } = createMtprotoDocumentStream(
      mtprotoSession.session_data,
      storedJson,
      { start: 0, end: 512 * 1024 - 1 },
      onDocumentRefreshed,
    );
    const audioPrefix = await readStreamPrefix(stream);
    const embedded = await extractEmbeddedCover(
      audioPrefix,
      stored.mimeType ?? track.mime_type ?? mimeType,
    );

    if (!embedded) {
      return NextResponse.json({ error: "No thumbnail" }, { status: 404 });
    }

    return new Response(bufferToStream(embedded.data), {
      status: 200,
      headers: thumbnailResponseHeaders(
        coverContentType(embedded.format),
        embedded.data.length,
      ),
    });
  } catch {
    return NextResponse.json(
      { error: "Failed to load thumbnail from Telegram" },
      { status: 502 },
    );
  }
}
