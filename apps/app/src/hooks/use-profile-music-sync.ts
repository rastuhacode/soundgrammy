import { useEffect, useState } from "react";
import { api, onSyncDone, onSyncStart } from "@/lib/api";

export type SyncPhase = "connecting" | "syncing" | "live";

function formatLastSync(value: string | null | undefined): string | null {
  if (!value) return null;
  // Backend stores UTC "YYYY-MM-DD HH:MM:SS"; normalize to ISO for Date.
  const iso = value.includes("T") ? value : `${value.replace(" ", "T")}Z`;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return null;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function useProfileMusicSync(_trackCount: number) {
  const [lastSyncAt, setLastSyncAt] = useState<string | null | undefined>(
    undefined,
  );
  const [syncing, setSyncing] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .syncStatus()
      .then((value) => {
        if (!cancelled) setLastSyncAt(value);
      })
      .catch(() => {
        if (!cancelled) setLastSyncAt(null);
      });

    const startPromise = onSyncStart(() => setSyncing(true));
    const donePromise = onSyncDone(() => {
      setSyncing(false);
      api
        .syncStatus()
        .then((value) => setLastSyncAt(value))
        .catch(() => {});
    });

    return () => {
      cancelled = true;
      void startPromise.then((unlisten) => unlisten());
      void donePromise.then((unlisten) => unlisten());
    };
  }, []);

  const phase: SyncPhase =
    lastSyncAt === undefined ? "connecting" : syncing ? "syncing" : "live";

  const lastSynced = formatLastSync(lastSyncAt);

  const statusLabel =
    phase === "connecting" ? "connecting" : phase === "syncing" ? "syncing" : "live";

  const statusDetail =
    phase === "connecting"
      ? "Connecting to Telegram…"
      : phase === "syncing"
        ? "Pulling your library…"
        : lastSynced
          ? `Last synced ${lastSynced}`
          : "Connected and ready";

  return { phase, statusLabel, statusDetail };
}
