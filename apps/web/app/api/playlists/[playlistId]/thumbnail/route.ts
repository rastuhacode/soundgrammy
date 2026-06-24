import { NextResponse } from "next/server";
import { getSession } from "@/lib/auth";
import { getPlaylistThumbnail } from "@/lib/db";

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
  { params }: { params: Promise<{ playlistId: string }> },
) {
  const session = await getSession();

  if (!session) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  const { playlistId } = await params;
  const id = Number(playlistId);
  if (!Number.isFinite(id)) {
    return NextResponse.json({ error: "Invalid playlist id" }, { status: 400 });
  }

  const thumbnail = getPlaylistThumbnail(id, session.tgUserId);
  if (!thumbnail) {
    return NextResponse.json({ error: "No thumbnail" }, { status: 404 });
  }

  return new Response(bufferToStream(thumbnail.data), {
    status: 200,
    headers: {
      "Content-Type": thumbnail.mime,
      "Cache-Control": "private, max-age=86400",
      "Content-Length": String(thumbnail.data.length),
    },
  });
}
