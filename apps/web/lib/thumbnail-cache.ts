type ThumbnailCacheEntry =
  | { status: "ready"; url: string }
  | { status: "failed" };

const cache = new Map<number, ThumbnailCacheEntry>();
const inflight = new Map<number, Promise<ThumbnailCacheEntry>>();

function thumbnailUrl(trackId: number): string {
  return `/api/tracks/${trackId}/thumbnail`;
}

export function getCachedThumbnail(
  trackId: number,
): ThumbnailCacheEntry | undefined {
  return cache.get(trackId);
}

export function loadCachedThumbnail(
  trackId: number,
): Promise<ThumbnailCacheEntry> {
  const hit = cache.get(trackId);
  if (hit) {
    return Promise.resolve(hit);
  }

  const pending = inflight.get(trackId);
  if (pending) {
    return pending;
  }

  const request = fetch(thumbnailUrl(trackId))
    .then(async (response): Promise<ThumbnailCacheEntry> => {
      if (!response.ok) {
        const entry: ThumbnailCacheEntry = { status: "failed" };
        cache.set(trackId, entry);
        return entry;
      }

      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const entry: ThumbnailCacheEntry = { status: "ready", url };
      cache.set(trackId, entry);
      return entry;
    })
    .catch((): ThumbnailCacheEntry => {
      const entry: ThumbnailCacheEntry = { status: "failed" };
      cache.set(trackId, entry);
      return entry;
    })
    .finally(() => {
      inflight.delete(trackId);
    });

  inflight.set(trackId, request);
  return request;
}
