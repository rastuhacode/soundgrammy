import { NextResponse } from "next/server";
import { createSession, getSessionCookieOptions } from "lib/auth";
import { consumeQrSuccess, getQrAuthStatus } from "lib/mtproto/qr-auth";

export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const authToken = searchParams.get("authToken");

  if (!authToken) {
    return NextResponse.json({ error: "authToken is required" }, { status: 400 });
  }

  const status = getQrAuthStatus(authToken);

  if (status.status === "success" && status.user) {
    const user = consumeQrSuccess(authToken) ?? status.user;
    const token = await createSession({
      tgUserId: user.tgUserId,
      firstName: user.firstName,
      lastName: user.lastName,
      username: user.username,
    });

    const response = NextResponse.json({ status: "success" });
    response.cookies.set(getSessionCookieOptions(token));
    return response;
  }

  return NextResponse.json(status);
}
