import { NextResponse } from "next/server";
import { getSession } from "lib/auth";
import { getTrackByFileId } from "lib/db";

interface TelegramFileResponse {
  ok: boolean;
  result?: {
    file_path: string;
  };
}

export async function GET(
  _request: Request,
  { params }: { params: Promise<{ fileId: string }> },
) {
  const session = await getSession();

  if (!session) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  const { fileId } = await params;

  // Verify the track belongs to this user
  const track = getTrackByFileId(fileId, session.tgUserId);
  if (!track) {
    return NextResponse.json({ error: "Track not found" }, { status: 404 });
  }

  const botToken = process.env.BOT_TOKEN;
  if (!botToken) {
    return NextResponse.json(
      { error: "Server configuration error" },
      { status: 500 },
    );
  }

  // Get file path from Telegram Bot API
  const fileResponse = await fetch(
    `https://api.telegram.org/bot${botToken}/getFile?file_id=${fileId}`,
  );
  const fileData: TelegramFileResponse = await fileResponse.json();

  if (!fileData.ok || !fileData.result?.file_path) {
    return NextResponse.json(
      { error: "Failed to get file from Telegram" },
      { status: 502 },
    );
  }

  // Stream the file from Telegram
  const audioResponse = await fetch(
    `https://api.telegram.org/file/bot${botToken}/${fileData.result.file_path}`,
  );

  if (!audioResponse.ok || !audioResponse.body) {
    return NextResponse.json(
      { error: "Failed to stream audio" },
      { status: 502 },
    );
  }

  return new Response(audioResponse.body, {
    headers: {
      "Content-Type": "audio/mpeg",
      "Cache-Control": "private, max-age=3600",
      "Accept-Ranges": "bytes",
    },
  });
}
