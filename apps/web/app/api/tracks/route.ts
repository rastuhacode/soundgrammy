import { NextResponse } from "next/server";
import { getSession } from "lib/auth";
import {
  BotAudioPayload,
  deleteTrack,
  getTracksByUser,
  insertTrack,
} from "lib/db";

export async function GET() {
  const session = await getSession();

  if (!session) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  const tracks = getTracksByUser(session.tgUserId);
  return NextResponse.json(tracks);
}

export async function POST(request: Request) {
  const apiKey = request.headers.get("x-api-key");
  const expectedKey = process.env.BOT_API_KEY;

  if (!expectedKey || apiKey !== expectedKey) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const body = (await request.json()) as BotAudioPayload;

  const { tg_user_id, file_id, file_unique_id, title, performer, duration } =
    body;

  if (!tg_user_id || !file_id || !file_unique_id) {
    return NextResponse.json(
      { error: "Missing required fields: tg_user_id, file_id, file_unique_id" },
      { status: 400 },
    );
  }

  const track = insertTrack(body);

  return NextResponse.json(track, { status: 201 });
}

export async function DELETE(request: Request) {
  const body = await request.json();

  const { id } = body;

  if (!id) {
    return NextResponse.json(
      { error: "Missing required fields: id" },
      { status: 400 },
    );
  }

  deleteTrack(id);

  return NextResponse.json({ success: true }, { status: 200 });
}
