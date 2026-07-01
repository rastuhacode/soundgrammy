import { useEffect, useState } from "react";
import { api, fileSrc } from "@/lib/api";

export function useUserAvatar(enabled = true): string | null {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    if (!enabled) {
      setSrc(null);
      return;
    }

    let cancelled = false;
    api
      .getUserAvatar()
      .then((path) => {
        if (!cancelled) setSrc(path ? fileSrc(path) : null);
      })
      .catch(() => {
        // Keep null — AvatarFallback shows initials.
      });

    return () => {
      cancelled = true;
    };
  }, [enabled]);

  return src;
}
