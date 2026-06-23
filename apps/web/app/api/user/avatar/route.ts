import { NextResponse } from "next/server";
import { getSession } from "@/lib/auth";
import { getMtprotoSession } from "@/lib/db";
import { downloadUserProfilePhoto } from "@/lib/mtproto/profile-photo";

function avatarResponseHeaders(contentLength: number): Record<string, string> {
  return {
    "Content-Type": "image/jpeg",
    "Cache-Control": "private, max-age=86400",
    "Content-Length": String(contentLength),
  };
}

export async function GET() {
  const session = await getSession();

  if (!session) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  const mtprotoSession = getMtprotoSession(session.tgUserId);
  if (!mtprotoSession) {
    return NextResponse.json(
      { error: "MTProto session required" },
      { status: 400 },
    );
  }

  try {
    const photo = await downloadUserProfilePhoto(mtprotoSession.session_data);
    if (!photo) {
      return NextResponse.json({ error: "No profile photo" }, { status: 404 });
    }

    return new Response(new Uint8Array(photo), {
      status: 200,
      headers: avatarResponseHeaders(photo.length),
    });
  } catch {
    return NextResponse.json(
      { error: "Failed to load profile photo from Telegram" },
      { status: 502 },
    );
  }
}
