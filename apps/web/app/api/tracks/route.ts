import { NextResponse } from "next/server";
import { getSession } from "lib/auth";
import { getTracksByUser, insertTrack } from "lib/db";

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

  const body = await request.json();

  const { tg_user_id, file_id, file_unique_id, title, performer, duration } =
    body;

  if (!tg_user_id || !file_id || !file_unique_id) {
    return NextResponse.json(
      { error: "Missing required fields: tg_user_id, file_id, file_unique_id" },
      { status: 400 },
    );
  }

  const track = insertTrack({
    tg_user_id,
    file_id,
    file_unique_id,
    title: title ?? null,
    performer: performer ?? null,
    duration: duration ?? null,
  });

  return NextResponse.json(track, { status: 201 });
}
