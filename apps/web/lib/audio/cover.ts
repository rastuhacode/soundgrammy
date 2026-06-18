import { parseBuffer } from "music-metadata";

/** Read enough of the stream to locate ID3/APIC tags (typical tags sit in the first ~512 KiB). */
const ID3_SCAN_BYTES = 512 * 1024;

export async function extractEmbeddedCover(
  audioBytes: Buffer,
  mimeType = "audio/mpeg",
): Promise<{ data: Buffer; format: string } | null> {
  const metadata = await parseBuffer(audioBytes, { mimeType });
  const picture = metadata.common.picture?.[0]; // first embedded cover art
  if (!picture) return null;
  return { data: Buffer.from(picture.data), format: picture.format };
}

export async function readStreamPrefix(
  stream: ReadableStream<Uint8Array>,
  maxBytes = ID3_SCAN_BYTES,
): Promise<Buffer> {
  const reader = stream.getReader();
  const chunks: Buffer[] = [];
  let total = 0;

  try {
    while (total < maxBytes) {
      const { done, value } = await reader.read();
      if (done || !value) {
        break;
      }

      const remaining = maxBytes - total;
      if (value.length > remaining) {
        chunks.push(Buffer.from(value.subarray(0, remaining)));
        break;
      }

      chunks.push(Buffer.from(value));
      total += value.length;
    }
  } finally {
    await reader.cancel();
  }

  return Buffer.concat(chunks);
}
