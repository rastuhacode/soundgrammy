import { NextResponse } from "next/server";
import { createSession, getSessionCookieOptions } from "lib/auth";
import {
  deleteMtprotoAuthPending,
  getMtprotoAuthPending,
} from "lib/db";
import { signInWithPassword } from "lib/mtproto/auth";
import { finalizeMtprotoLogin } from "lib/mtproto/login";

export async function POST(request: Request) {
  const body = (await request.json()) as {
    authToken?: string;
    password?: string;
  };

  const { authToken, password } = body;
  if (!authToken || !password) {
    return NextResponse.json(
      { error: "authToken and password are required" },
      { status: 400 },
    );
  }

  const pending = getMtprotoAuthPending(authToken);
  if (!pending) {
    return NextResponse.json(
      { error: "Auth session expired. Request a new code." },
      { status: 410 },
    );
  }

  try {
    const result = await signInWithPassword(password, pending.session_data);
    const user = await finalizeMtprotoLogin(
      result.sessionData,
      pending.phone_number,
    );
    deleteMtprotoAuthPending(authToken);

    const token = await createSession({
      tgUserId: user.tgUserId,
      firstName: user.firstName,
      lastName: user.lastName,
      username: user.username,
    });

    const response = NextResponse.json({ ok: true });
    response.cookies.set(getSessionCookieOptions(token));
    return response;
  } catch (error) {
    const message =
      error instanceof Error ? error.message : "Invalid password";
    return NextResponse.json({ error: message }, { status: 401 });
  }
}
