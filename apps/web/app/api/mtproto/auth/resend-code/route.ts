import { NextResponse } from "next/server";
import {
  getMtprotoAuthPending,
  updateMtprotoAuthPending,
} from "lib/db";
import { resendAuthCode } from "lib/mtproto/auth";

export async function POST(request: Request) {
  const body = (await request.json()) as { authToken?: string };
  const { authToken } = body;

  if (!authToken) {
    return NextResponse.json(
      { error: "authToken is required" },
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
    const result = await resendAuthCode(
      pending.phone_number,
      pending.phone_code_hash,
      pending.session_data,
    );

    updateMtprotoAuthPending(
      authToken,
      result.phoneCodeHash,
      result.sessionData,
    );

    return NextResponse.json({ codeDelivery: result.codeDelivery });
  } catch (error) {
    const message =
      error instanceof Error ? error.message : "Failed to resend code";
    return NextResponse.json({ error: message }, { status: 502 });
  }
}
