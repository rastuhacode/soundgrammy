import { NextResponse } from "next/server";
import { getSession } from "lib/auth";
import { getMtprotoSession, getTrackById } from "lib/db";
import {
  createMtprotoDocumentStream,
  parseStoredDocument,
} from "lib/mtproto/client";

function parseByteRange(
  rangeHeader: string,
  fileSize: number,
): { start: number; end: number } | "unsatisfiable" | null {
  if (!rangeHeader.startsWith("bytes=")) {
    return null;
  }

  const [startStr, endStr] = rangeHeader.slice(6).split("-");
  if (startStr === "" && endStr === "") {
    return "unsatisfiable";
  }

  let start: number;
  let end: number;

  if (startStr === "") {
    const suffixLength = Number(endStr);
    if (!Number.isFinite(suffixLength) || suffixLength <= 0) {
      return "unsatisfiable";
    }
    start = Math.max(0, fileSize - suffixLength);
    end = fileSize - 1;
  } else {
    start = Number(startStr);
    end = endStr !== "" ? Number(endStr) : fileSize - 1;
  }

  if (!Number.isFinite(start) || !Number.isFinite(end)) {
    return "unsatisfiable";
  }
  if (start >= fileSize || start > end) {
    return "unsatisfiable";
  }

  end = Math.min(end, fileSize - 1);
  return { start, end };
}

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
  const totalSize =
    Number(storedDocument.size ?? 0) || Number(track.file_size ?? 0);
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

  const byteRange = parsedRange ?? { start: 0, end: totalSize - 1 };

  try {
    const { stream, mimeType, contentLength } = createMtprotoDocumentStream(
      mtprotoSession.session_data,
      track.mtproto_document,
      byteRange,
    );

    const headers: Record<string, string> = {
      "Content-Type": mimeType,
      "Accept-Ranges": "bytes",
      "Cache-Control": "private, max-age=3600",
      "Content-Length": String(contentLength),
    };

    if (parsedRange) {
      headers["Content-Range"] =
        `bytes ${byteRange.start}-${byteRange.end}/${totalSize}`;
      return new Response(stream, { status: 206, headers });
    }

    return new Response(stream, { status: 200, headers });
  } catch {
    return NextResponse.json(
      { error: "Failed to stream audio from Telegram" },
      { status: 502 },
    );
  }
}
