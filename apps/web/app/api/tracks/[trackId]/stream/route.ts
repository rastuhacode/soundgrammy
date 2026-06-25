import { NextResponse } from "next/server";
import { getSession } from "@/lib/auth";
import { mtprotoRouteErrorResponse } from "@/lib/api/auth/server/mtproto-route-error";
import {
  getMtprotoSession,
  getTrackById,
  updateTrackMtprotoDocument,
} from "@/lib/db";
import {
  createMtprotoDocumentStream,
  parseStoredDocument,
} from "@/lib/mtproto/client";
import { parseByteRange } from "@/lib/stream/range";
import { readFirstStreamChunk } from "@/lib/stream/preflight";

export async function GET(
  request: Request,
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

  const storedDocument = parseStoredDocument(track.mtproto_document);
  const totalSize
    = Number(storedDocument.size ?? 0) || Number(track.file_size ?? 0);
  if (totalSize <= 0) {
    return NextResponse.json(
      { error: "Track file size is unknown" },
      { status: 400 },
    );
  }

  const rangeHeader = request.headers.get("range");
  const parsedRange = rangeHeader
    ? parseByteRange(rangeHeader, totalSize)
    : null;

  if (parsedRange === "unsatisfiable") {
    return new Response(null, {
      status: 416,
      headers: {
        "Content-Range": `bytes */${totalSize}`,
      },
    });
  }

  // Serve the whole requested range in a single response and let the stream's
  // backpressure decide how much is actually pulled from Telegram: the browser
  // reads only its current position plus its own look-ahead, then stops reading
  // (pausing our Telegram download) until playback advances. Sequential
  // playback is therefore one open request; a new request is opened only on
  // seek (a fresh Range) — no artificial chunking that would stall playback.
  const isRangeRequest = parsedRange !== null;
  const byteRange = parsedRange ?? { start: 0, end: totalSize - 1 };

  try {
    const { stream, mimeType, contentLength } = createMtprotoDocumentStream(
      mtprotoSession.session_data,
      track.mtproto_document,
      byteRange,
      (refreshedDocument) => {
        updateTrackMtprotoDocument(
          track.id,
          session.tgUserId,
          refreshedDocument,
        );
      },
    );

    const opened = await readFirstStreamChunk(stream);
    if (!opened.ok) {
      return mtprotoRouteErrorResponse(
        opened.error,
        "Failed to stream audio from Telegram",
      );
    }

    const headers: Record<string, string> = {
      "Content-Type": mimeType,
      "Accept-Ranges": "bytes",
      "Cache-Control": "private, max-age=3600",
      "Content-Length": String(contentLength),
    };

    if (isRangeRequest) {
      headers["Content-Range"]
        = `bytes ${byteRange.start}-${byteRange.end}/${totalSize}`;
      return new Response(opened.stream, { status: 206, headers });
    }

    return new Response(opened.stream, { status: 200, headers });
  } catch (error) {
    return mtprotoRouteErrorResponse(
      error,
      "Failed to stream audio from Telegram",
    );
  }
}
