/**
 * Pulls the first chunk from a lazy stream so Telegram auth errors surface
 * before the HTTP response is committed. Uses the same download path as normal
 * streaming — no separate session/status probe.
 */
export async function readFirstStreamChunk(
  stream: ReadableStream<Uint8Array>,
): Promise<
  | { ok: true; stream: ReadableStream<Uint8Array> }
  | { ok: false; error: unknown }
> {
  const reader = stream.getReader();

  try {
    const first = await reader.read();

    if (first.done) {
      reader.releaseLock();
      return {
        ok: true,
        stream: new ReadableStream({
          start(controller) {
            controller.close();
          },
        }),
      };
    }

    const resumed = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(first.value);
      },
      async pull(controller) {
        while (true) {
          const next = await reader.read();
          if (next.done) {
            reader.releaseLock();
            controller.close();
            return;
          }
          controller.enqueue(next.value);
        }
      },
      async cancel(reason) {
        await reader.cancel(reason);
        reader.releaseLock();
      },
    });

    return { ok: true, stream: resumed };
  } catch (error) {
    try {
      await reader.cancel();
    } catch {
      // ignore cancel failures
    }
    reader.releaseLock();
    return { ok: false, error };
  }
}
