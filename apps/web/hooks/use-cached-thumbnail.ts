import { useEffect, useState } from "react";
import {
  getCachedThumbnail,
  loadCachedThumbnail,
} from "@/lib/thumbnail-cache";

interface CachedThumbnailState {
  url: string | null;
  loaded: boolean;
  failed: boolean;
}

interface UseCachedThumbnailOptions {
  /** When false, skips the network fetch until enabled (e.g. row is in view). */
  enabled?: boolean;
}

function stateFromCache(trackId: number): CachedThumbnailState {
  const cached = getCachedThumbnail(trackId);
  if (cached?.status === "ready") {
    return { url: cached.url, loaded: true, failed: false };
  }
  if (cached?.status === "failed") {
    return { url: null, loaded: false, failed: true };
  }
  return { url: null, loaded: false, failed: false };
}

export function useCachedThumbnail(
  trackId: number,
  options: UseCachedThumbnailOptions = {},
): CachedThumbnailState {
  const enabled = options.enabled ?? true;
  const [state, setState] = useState(() => stateFromCache(trackId));

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const cached = getCachedThumbnail(trackId);
    if (cached) {
      setState(stateFromCache(trackId));
      return;
    }

    let cancelled = false;

    loadCachedThumbnail(trackId).then((entry) => {
      if (cancelled) {
        return;
      }

      if (entry.status === "ready") {
        setState({ url: entry.url, loaded: true, failed: false });
        return;
      }

      setState({ url: null, loaded: false, failed: true });
    });

    return () => {
      cancelled = true;
    };
  }, [trackId, enabled]);

  return state;
}
