import { useEffect, useState } from "react";
import { api } from "@/lib/api";

// Custom playlist covers are stored as base64 in SQLite and returned as a
// ready-to-use `data:` URL, so we just fetch and hold it.
export function usePlaylistThumbnail(
  playlistId: number | undefined,
  hasThumbnail: boolean,
): string | null {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect -- Cover image state mirrors an async backend lookup keyed by playlistId. */
    if (playlistId === undefined || !hasThumbnail) {
      setSrc(null);
      return;
    }

    let cancelled = false;
    api
      .getPlaylistThumbnail(playlistId)
      .then((dataUrl) => {
        if (!cancelled) setSrc(dataUrl);
      })
      .catch(() => {
        if (!cancelled) setSrc(null);
      });

    return () => {
      cancelled = true;
    };
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [playlistId, hasThumbnail]);

  return src;
}
