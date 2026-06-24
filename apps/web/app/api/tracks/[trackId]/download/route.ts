import { NextResponse } from "next/server";
import { getSession } from "@/lib/auth";
import { buildTrackDownloadFilename } from "@/lib/download-filename";
import {
  getMtprotoSession,
  getTrackById,
  updateTrackMtprotoDocument,
} from "@/lib/db";
import {
  createMtprotoDocumentStream,
  parseStoredDocument,
  resolveTrackDocumentMetadata,
} from "@/lib/mtproto/client";
import { getFilenameFromDocumentAttributes } from "@/lib/mtproto/document";

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

  const storedDocument = parseStoredDocument(track.mtproto_document);
  const totalSize
    = Number(storedDocument.size ?? 0) || Number(track.file_size ?? 0);
  if (totalSize <= 0) {
    return NextResponse.json(
      { error: "Track file size is unknown" },
      { status: 400 },
    );
  }

  try {
    let attributeFileName = getFilenameFromDocumentAttributes(
      storedDocument.attributes,
    );
    if (!attributeFileName) {
      const metadata = await resolveTrackDocumentMetadata(
        mtprotoSession.session_data,
        track.mtproto_document,
      );
      attributeFileName = getFilenameFromDocumentAttributes(
        metadata.attributes,
      );
    }

    const { stream, mimeType, contentLength } = createMtprotoDocumentStream(
      mtprotoSession.session_data,
      track.mtproto_document,
      { start: 0, end: totalSize - 1 },
      (refreshedDocument) => {
        updateTrackMtprotoDocument(
          track.id,
          session.tgUserId,
          refreshedDocument,
        );
      },
    );

    const filename = buildTrackDownloadFilename({
      attributeFileName,
      title: track.title,
      fileName: track.file_name,
      trackId: track.id,
      mimeType: storedDocument.mimeType ?? track.mime_type ?? mimeType,
    });

    return new Response(stream, {
      status: 200,
      headers: {
        "Content-Type": mimeType,
        "Content-Length": String(contentLength),
        "Content-Disposition": `attachment; filename="${filename}"`,
        "Cache-Control": "private, no-store",
      },
    });
  } catch {
    return NextResponse.json(
      { error: "Failed to download audio from Telegram" },
      { status: 502 },
    );
  }
}
