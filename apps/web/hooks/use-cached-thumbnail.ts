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

export function useCachedThumbnail(trackId: number): CachedThumbnailState {
  const [state, setState] = useState(() => stateFromCache(trackId));

  useEffect(() => {
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
  }, [trackId]);

  return state;
}
