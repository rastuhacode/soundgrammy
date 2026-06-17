import { NextResponse } from "next/server";
import { getSession } from "lib/auth";
import { deleteMtprotoSession } from "lib/db";

export async function POST() {
  const session = await getSession();

  const response = NextResponse.json({ ok: true });
  response.cookies.delete("session");

  if (session) {
    deleteMtprotoSession(session.tgUserId);
  }

  return response;
}
