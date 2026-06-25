"use client";

import { useEffect, useState } from "react";
import { useFetch } from "@/lib/api/auth/client/use-fetch";

const USER_AVATAR_URL = "/api/user/avatar";

export function useUserAvatar(enabled = true): string | null {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    if (!enabled) {
      setSrc(null);
      return;
    }

    let objectUrl: string | null = null;
    let cancelled = false;

    void (async () => {
      try {
        const response = await useFetch(USER_AVATAR_URL);
        if (!response.ok || cancelled) {
          return;
        }
        const blob = await response.blob();
        if (cancelled) {
          return;
        }
        objectUrl = URL.createObjectURL(blob);
        setSrc(objectUrl);
      } catch {
        // Keep src null — AvatarFallback shows initials.
      }
    })();

    return () => {
      cancelled = true;
      if (objectUrl) {
        URL.revokeObjectURL(objectUrl);
      }
    };
  }, [enabled]);

  return src;
}
