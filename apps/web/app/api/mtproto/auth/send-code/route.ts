import { randomUUID } from "node:crypto";
import { NextResponse } from "next/server";
import { createMtprotoAuthPending } from "lib/db";
import { sendAuthCode } from "lib/mtproto/auth";
import { normalizePhoneNumber } from "lib/mtproto/phone";

export async function POST(request: Request) {
  const body = (await request.json()) as {
    phoneNumber?: string;
    forceSms?: boolean;
  };

  let phoneNumber: string;
  try {
    phoneNumber = normalizePhoneNumber(body.phoneNumber?.trim() ?? "");
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : "Invalid phone number" },
      { status: 400 },
    );
  }

  try {
    const { phoneCodeHash, sessionData, codeDelivery } = await sendAuthCode(
      phoneNumber,
      body.forceSms ?? false,
    );
    const authToken = randomUUID();
    createMtprotoAuthPending(
      authToken,
      phoneNumber,
      phoneCodeHash,
      sessionData,
    );

    return NextResponse.json({ authToken, codeDelivery });
  } catch (error) {
    const message =
      error instanceof Error ? error.message : "Failed to send auth code";
    return NextResponse.json({ error: message }, { status: 502 });
  }
}
