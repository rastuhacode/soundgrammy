"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";

interface MtprotoStatus {
  connected: boolean;
  phoneNumber: string | null;
  lastSyncAt: string | null;
}

interface ProfileMusicSyncProps {
  trackCount: number;
}

export function ProfileMusicSync({ trackCount }: ProfileMusicSyncProps) {
  const router = useRouter();
  const [status, setStatus] = useState<MtprotoStatus | null>(null);
  const [loggingOut, setLoggingOut] = useState(false);
  const [syncing, setSyncing] = useState(trackCount === 0);
  const backgroundSyncStarted = useRef(false);

  const loadStatus = useCallback(async () => {
    const response = await fetch("/api/mtproto/status");
    if (!response.ok) return;
    const data = (await response.json()) as MtprotoStatus;
    setStatus(data);
  }, []);

  const runSync = useCallback(async () => {
    setSyncing(true);
    const response = await fetch("/api/mtproto/sync", { method: "POST" });
    setSyncing(false);
    if (!response.ok) {
      return false;
    }

    await loadStatus();
    router.refresh();
    return true;
  }, [loadStatus, router]);

  useEffect(() => {
    if (trackCount > 0) {
      setSyncing(false);
    }
  }, [trackCount]);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  useEffect(() => {
    if (!status?.connected || backgroundSyncStarted.current) {
      return;
    }

    backgroundSyncStarted.current = true;
    void runSync();
  }, [status?.connected, runSync]);

  const handleLogout = async () => {
    setLoggingOut(true);
    await fetch("/api/auth/logout", { method: "POST" });
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
          disabled={loggingOut}
        >
          Log out
        </Button>
      </div>
    </section>
  );
}
