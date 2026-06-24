type ThumbnailCacheEntry
  = | { status: "ready"; url: string }
    | { status: "failed" };

const cache = new Map<string, ThumbnailCacheEntry>();
const inflight = new Map<string, Promise<ThumbnailCacheEntry>>();

function thumbnailUrl(trackId: number): string {
  return `/api/tracks/${trackId}/thumbnail`;
}

export function clearThumbnailCache(): void {
  for (const entry of cache.values()) {
    if (entry.status === "ready") {
      URL.revokeObjectURL(entry.url);
    }
  }
  cache.clear();
  inflight.clear();
}

export function getCachedThumbnail(
  fileUniqueId: string,
): ThumbnailCacheEntry | undefined {
  return cache.get(fileUniqueId);
}

export function loadCachedThumbnail(
  fileUniqueId: string,
  trackId: number,
): Promise<ThumbnailCacheEntry> {
  const hit = cache.get(fileUniqueId);
  if (hit) {
    return Promise.resolve(hit);
  }

  const pending = inflight.get(fileUniqueId);
  if (pending) {
    return pending;
  }

  const request = fetch(thumbnailUrl(trackId))
    .then(async (response): Promise<ThumbnailCacheEntry> => {
      if (!response.ok) {
        const entry: ThumbnailCacheEntry = { status: "failed" };
        cache.set(fileUniqueId, entry);
        return entry;
      }

      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const entry: ThumbnailCacheEntry = { status: "ready", url };
      cache.set(fileUniqueId, entry);
      return entry;
    })
    .catch((): ThumbnailCacheEntry => {
      const entry: ThumbnailCacheEntry = { status: "failed" };
      cache.set(fileUniqueId, entry);
      return entry;
    })
    .finally(() => {
      inflight.delete(fileUniqueId);
    });

  inflight.set(fileUniqueId, request);
  return request;
}
