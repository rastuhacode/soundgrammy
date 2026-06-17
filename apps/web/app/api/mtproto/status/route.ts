import { NextResponse } from "next/server";
import { getSession } from "lib/auth";
import { getMtprotoSession } from "lib/db";

export async function GET() {
  const session = await getSession();
  if (!session) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  const mtprotoSession = getMtprotoSession(session.tgUserId);

  return NextResponse.json({
    connected: Boolean(mtprotoSession),
    phoneNumber: mtprotoSession?.phone_number ?? null,
    lastSyncAt: mtprotoSession?.last_sync_at ?? null,
  });
}
