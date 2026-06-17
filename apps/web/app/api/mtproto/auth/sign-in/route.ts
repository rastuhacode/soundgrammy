import { NextResponse } from "next/server";
import { createSession, getSessionCookieOptions } from "lib/auth";
import {
  deleteMtprotoAuthPending,
  getMtprotoAuthPending,
  updateMtprotoAuthPendingSession,
} from "lib/db";
import { signInWithCode } from "lib/mtproto/auth";
import { finalizeMtprotoLogin } from "lib/mtproto/login";

export async function POST(request: Request) {
  const body = (await request.json()) as {
    authToken?: string;
    code?: string;
  };

  const { authToken, code } = body;
  if (!authToken || !code) {
    return NextResponse.json(
      { error: "authToken and code are required" },
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
    const result = await signInWithCode(
      pending.phone_number,
      pending.phone_code_hash,
      code.trim(),
      pending.session_data,
    );

    if (result.needsPassword) {
      updateMtprotoAuthPendingSession(authToken, result.sessionData);
      return NextResponse.json({ needsPassword: true });
    }

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
      error instanceof Error ? error.message : "Failed to sign in";
    return NextResponse.json({ error: message }, { status: 401 });
  }
}
