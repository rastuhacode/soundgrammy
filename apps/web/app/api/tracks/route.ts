import { NextResponse } from "next/server";
import { getSession } from "lib/auth";
import { deleteTrack, getTracksByUser } from "lib/db";

export async function GET() {
  const session = await getSession();

  if (!session) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  const tracks = getTracksByUser(session.tgUserId);
  return NextResponse.json(tracks);
}

export async function DELETE(request: Request) {
  const session = await getSession();
  if (!session) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  const body = await request.json();

  const { id } = body;

  if (!id) {
    return NextResponse.json(
      { error: "Missing required fields: id" },
      { status: 400 },
    );
  }

  deleteTrack(id, session.tgUserId);

  return NextResponse.json({ success: true }, { status: 200 });
}
