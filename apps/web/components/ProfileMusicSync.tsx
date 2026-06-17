"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";

interface MtprotoStatus {
  connected: boolean;
  phoneNumber: string | null;
  lastSyncAt: string | null;
}

interface ProfileMusicSyncProps {
  onSynced?: () => void;
}

export function ProfileMusicSync({ onSynced }: ProfileMusicSyncProps) {
  const router = useRouter();
  const [status, setStatus] = useState<MtprotoStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [loggingOut, setLoggingOut] = useState(false);

  const loadStatus = useCallback(async () => {
    const response = await fetch("/api/mtproto/status");
    if (!response.ok) return;
    const data = (await response.json()) as MtprotoStatus;
    setStatus(data);
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const handleSync = async () => {
    setSyncing(true);
    setError(null);

    const response = await fetch("/api/mtproto/sync", { method: "POST" });
    const data = await response.json();
    setSyncing(false);

    if (!response.ok) {
      setError(data.error ?? "Sync failed");
      return;
    }

    await loadStatus();
    onSynced?.();
    router.refresh();
  };

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
              : "Sync songs pinned to your Telegram profile"}
          </p>
          {status.lastSyncAt ? (
            <p className="text-xs opacity-50 mt-1">
              Last sync: {new Date(status.lastSyncAt).toLocaleString()}
            </p>
          ) : null}
        </div>
        <div className="flex flex-wrap gap-2">
          <Button onClick={handleSync} disabled={syncing || !status.connected}>
            {syncing ? "Syncing…" : "Sync profile music"}
          </Button>
          <Button
            variant="outline"
            onClick={handleLogout}
            disabled={loggingOut}
          >
            Log out
          </Button>
        </div>
      </div>

      {error ? (
        <p className="text-sm text-destructive" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
