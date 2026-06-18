import { NextResponse } from "next/server";
import { getSession } from "lib/auth";
import {
  extractEmbeddedCover,
  readStreamPrefix,
} from "lib/audio/cover";
import {
  getMtprotoSession,
  getTrackById,
  updateTrackMtprotoDocument,
} from "lib/db";
import {
  createMtprotoDocumentStream,
  downloadMtprotoDocumentThumbnail,
  withMtprotoClient,
} from "lib/mtproto/client";
import { enrichSavedMusicDocumentThumb } from "lib/mtproto/refresh";
import {
  parseStoredDocument,
  shouldUpgradeStoredThumb,
} from "lib/mtproto/document";

function coverContentType(format: string): string {
  if (format.includes("png")) return "image/png";
  if (format.includes("webp")) return "image/webp";
  return "image/jpeg";
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
          enrichSavedMusicDocumentThumb(client, parseStoredDocument(storedJson)),
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

  try {
    if (stored.thumbSize) {
      const thumbnail = await downloadMtprotoDocumentThumbnail(
        mtprotoSession.session_data,
        storedJson,
        (refreshedDocument) => {
          updateTrackMtprotoDocument(
            track.id,
            session.tgUserId,
            refreshedDocument,
          );
        },
      );

      if (thumbnail && thumbnail.length > 0) {
        return new Response(new Uint8Array(thumbnail), {
          status: 200,
          headers: {
            "Content-Type": "image/jpeg",
            "Cache-Control": "private, max-age=86400",
          },
        });
      }
    }

    if (stored.thumbData) {
      const thumbnail = Buffer.from(stored.thumbData, "base64");
      return new Response(new Uint8Array(thumbnail), {
        status: 200,
        headers: {
          "Content-Type": "image/jpeg",
          "Cache-Control": "private, max-age=86400",
        },
      });
    }

    const { stream, mimeType } = createMtprotoDocumentStream(
      mtprotoSession.session_data,
      storedJson,
      { start: 0, end: 512 * 1024 - 1 },
      (refreshedDocument) => {
        updateTrackMtprotoDocument(track.id, session.tgUserId, refreshedDocument);
      },
    );
    const audioPrefix = await readStreamPrefix(stream);
    const embedded = await extractEmbeddedCover(
      audioPrefix,
      stored.mimeType ?? track.mime_type ?? mimeType,
    );

    if (!embedded) {
      return NextResponse.json({ error: "No thumbnail" }, { status: 404 });
    }

    return new Response(new Uint8Array(embedded.data), {
      status: 200,
      headers: {
        "Content-Type": coverContentType(embedded.format),
        "Cache-Control": "private, max-age=86400",
      },
    });
  } catch {
    return NextResponse.json(
      { error: "Failed to load thumbnail from Telegram" },
      { status: 502 },
    );
  }
}
