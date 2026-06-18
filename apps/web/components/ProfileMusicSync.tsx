"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { useTRPC } from "../trpc/client";

interface ProfileMusicSyncProps {
  trackCount: number;
}

export function ProfileMusicSync({ trackCount }: ProfileMusicSyncProps) {
  const router = useRouter();
  const trpc = useTRPC();

  const statusQuery = useQuery(trpc.mtproto.status.queryOptions());
  const syncMutation = useMutation(trpc.mtproto.sync.mutationOptions());
  const logoutMutation = useMutation(trpc.auth.logout.mutationOptions());

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

  const handleLogout = async () => {
    await logoutMutation.mutateAsync();
    router.push("/login");
    router.refresh();
  };

  if (status === null) {
    return (
      <div className="mx-4 p-4 rounded-lg border border-border bg-card text-sm opacity-60">
        Loading…
      </div>
    );
  }

  return (
    <section className="mx-4 p-4 rounded-lg border border-border bg-card space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold">Profile music</h2>
          <p className="text-sm opacity-60 mt-1">
            {status.phoneNumber
              ? `Signed in as ${status.phoneNumber}`
              : "Songs pinned to your Telegram profile"}
          </p>
          {syncing ? (
            <p className="text-xs opacity-50 mt-1">Syncing your library…</p>
          ) : status.lastSyncAt ? (
            <p className="text-xs opacity-50 mt-1">
              Last synced: {new Date(status.lastSyncAt).toLocaleString()}
            </p>
          ) : null}
        </div>
        <Button
          variant="outline"
          onClick={handleLogout}
          disabled={logoutMutation.isPending}
        >
          Log out
        </Button>
      </div>
    </section>
  );
}
