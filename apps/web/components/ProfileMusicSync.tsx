"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { useMutation, useQuery } from "@tanstack/react-query";
import { LogOut, RadioTower } from "lucide-react";
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
      <div className="flex animate-pulse items-center gap-3 rounded-xl border border-border bg-card/60 px-5 py-4 text-sm text-muted-foreground">
        <span className="h-2 w-2 rounded-full bg-muted-foreground/60" />
        Connecting to Telegram…
      </div>
    );
  }

  return (
    <section className="animate-fade-up flex flex-wrap items-center justify-between gap-4 rounded-xl border border-border bg-card/60 px-5 py-4 backdrop-blur-sm">
      <div className="flex items-center gap-4">
        <div className="relative flex h-11 w-11 shrink-0 items-center justify-center rounded-lg border border-primary/30 bg-primary/10 text-primary">
          <RadioTower className="size-5" />
          {syncing ? (
            <span className="absolute -right-1 -top-1 flex h-3 w-3">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary opacity-75" />
              <span className="relative inline-flex h-3 w-3 rounded-full bg-primary" />
            </span>
          ) : null}
        </div>
        <div>
          <div className="flex items-center gap-2">
            <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-primary">
              {syncing ? "syncing" : "live"}
            </span>
          </div>
          {syncing ? (
            <p className="mt-1 font-mono text-xs text-muted-foreground/70">
              Pulling your library…
            </p>
          ) : status.lastSyncAt ? (
            <p className="mt-1 font-mono text-xs text-muted-foreground/70">
              Last synced {new Date(status.lastSyncAt).toString()}
            </p>
          ) : null}
        </div>
      </div>
      <Button
        variant="outline"
        size="sm"
        onClick={handleLogout}
        disabled={logoutMutation.isPending}
      >
        <LogOut className="size-4" />
        Log out
      </Button>
    </section>
  );
}
