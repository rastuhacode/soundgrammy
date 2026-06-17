import { NextResponse } from "next/server";
import { getSession } from "lib/auth";
import {
  getMtprotoSession,
  updateMtprotoLastSync,
} from "lib/db";
import { withMtprotoClient } from "lib/mtproto/client";
import { syncProfileMusic } from "lib/mtproto/sync";

export async function POST() {
  const session = await getSession();
  if (!session) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  const mtprotoSession = getMtprotoSession(session.tgUserId);
  if (!mtprotoSession) {
    return NextResponse.json(
      { error: "Connect your Telegram account first" },
      { status: 400 },
    );
  }

  try {
    const result = await withMtprotoClient(
      mtprotoSession.session_data,
      async (client) => syncProfileMusic(client, session.tgUserId),
    );

    updateMtprotoLastSync(session.tgUserId);

    return NextResponse.json(result);
  } catch (error) {
    const message =
      error instanceof Error ? error.message : "Failed to sync profile music";
    return NextResponse.json({ error: message }, { status: 502 });
  }
}
