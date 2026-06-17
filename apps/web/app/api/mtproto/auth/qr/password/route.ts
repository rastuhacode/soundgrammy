import { NextResponse } from "next/server";
import { getQrAuthStatus, submitQrPassword } from "lib/mtproto/qr-auth";

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

  try {
    submitQrPassword(authToken, password);
    return NextResponse.json({ ok: true });
  } catch (error) {
    const message =
      error instanceof Error ? error.message : "Failed to submit password";
    return NextResponse.json({ error: message }, { status: 400 });
  }
}

export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const authToken = searchParams.get("authToken");
  if (!authToken) {
    return NextResponse.json({ error: "authToken is required" }, { status: 400 });
  }
  return NextResponse.json(getQrAuthStatus(authToken));
}
