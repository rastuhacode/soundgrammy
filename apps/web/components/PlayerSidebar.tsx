"use client";

import { ProfileMusicSync } from "@/components/ProfileMusicSync";
import { usePlayerStore } from "@/stores/player-store";

export function PlayerSidebar() {
  const trackCount = usePlayerStore((state) => state.tracks.length);

  return (
    <div className="flex h-full flex-col gap-6 p-4">
      <div>
        <h1 className="font-[--font-fraunces] text-xl font-semibold tracking-tight text-foreground">
          SoundGrammy
        </h1>
        <p className="mt-1 text-xs text-muted-foreground">Your library</p>
      </div>
      <ProfileMusicSync trackCount={trackCount} />
    </div>
  );
}
