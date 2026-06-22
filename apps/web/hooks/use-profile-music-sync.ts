"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useTRPC } from "@/trpc/client";

export type SyncPhase = "connecting" | "syncing" | "live";

function formatLastSync(value: string | null | undefined): string | null {
  if (!value) {
    return null;
  }
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

export function useProfileMusicSync(trackCount: number) {
  const router = useRouter();
  const trpc = useTRPC();

  const statusQuery = useQuery(trpc.mtproto.status.queryOptions());
  const syncMutation = useMutation(trpc.mtproto.sync.mutationOptions());

  const status = statusQuery.data ?? null;
  const [syncing, setSyncing] = useState(trackCount === 0);
  const backgroundSyncStarted = useRef(false);

  const runSync = useCallback(async () => {
    setSyncing(true);
    try {
      await syncMutation.mutateAsync();
    } catch {
      setSyncing(false);
      return false;
    }
    setSyncing(false);
    await statusQuery.refetch();
    router.refresh();
    return true;
  }, [syncMutation, statusQuery, router]);

  useEffect(() => {
    if (trackCount > 0) {
      setSyncing(false);
    }
  }, [trackCount]);

  useEffect(() => {
    if (!status?.connected || backgroundSyncStarted.current) {
      return;
    }

    backgroundSyncStarted.current = true;
    void runSync();
  }, [status?.connected, runSync]);

  const phase: SyncPhase =
    status === null ? "connecting" : syncing ? "syncing" : "live";

  const lastSynced = formatLastSync(status?.lastSyncAt);

  const statusLabel =
    phase === "connecting"
      ? "connecting"
      : phase === "syncing"
        ? "syncing"
        : "live";

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
