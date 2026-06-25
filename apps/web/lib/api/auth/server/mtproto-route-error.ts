import { NextResponse } from "next/server";
import { isMtprotoSessionError } from "@/lib/mtproto/errors";

export function mtprotoRouteErrorResponse(
  error: unknown,
  fallbackMessage: string,
): NextResponse {
  if (isMtprotoSessionError(error)) {
    return NextResponse.json(
      { error: "Telegram session expired" },
      { status: 401 },
    );
  }

  return NextResponse.json({ error: fallbackMessage }, { status: 502 });
}
