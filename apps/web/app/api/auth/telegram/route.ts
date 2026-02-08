import { NextResponse } from "next/server";
import { verifyTelegramLogin, type TelegramLoginData } from "lib/telegram";
import { createSession, getSessionCookieOptions } from "lib/auth";

export async function POST(request: Request) {
  const data: TelegramLoginData = await request.json();

  if (!verifyTelegramLogin(data)) {
    return NextResponse.json(
      { error: "Invalid Telegram auth data" },
      { status: 401 },
    );
  }

  const token = await createSession({
    tgUserId: data.id,
    firstName: data.first_name,
    lastName: data.last_name,
    username: data.username,
    photoUrl: data.photo_url,
  });

  const response = NextResponse.json({ ok: true });
  response.cookies.set(getSessionCookieOptions(token));
  return response;
}
