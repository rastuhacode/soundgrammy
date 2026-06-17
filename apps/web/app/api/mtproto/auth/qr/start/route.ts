import { randomUUID } from "node:crypto";
import QRCode from "qrcode";
import { NextResponse } from "next/server";
import { startQrAuth } from "lib/mtproto/qr-auth";

export async function POST() {
  try {
    const authToken = randomUUID();
    const { qrUrl, expires } = await startQrAuth(authToken);
    const qrDataUrl = await QRCode.toDataURL(qrUrl, {
      width: 280,
      margin: 1,
      color: { dark: "#000000", light: "#ffffff" },
    });

    return NextResponse.json({
      authToken,
      qrUrl,
      qrDataUrl,
      expires,
    });
  } catch (error) {
    const message =
      error instanceof Error ? error.message : "Failed to start QR login";
    return NextResponse.json({ error: message }, { status: 502 });
  }
}
