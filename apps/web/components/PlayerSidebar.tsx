"use client";

import { SidebarProfile } from "@/components/SidebarProfile";
import { SidebarPlaylists } from "@/components/SidebarPlaylists";
import { Separator } from "@/components/ui/separator";
import { useLibraryStore } from "@/stores/library-store";

export function PlayerSidebar() {
  const trackCount = useLibraryStore((state) => state.tracks.length);

  return (
    <div className="flex h-full flex-col gap-4 pt-4">
      <div className="flex w-full items-center justify-between gap-2 px-4 h-10">
        <h1 className="text-2xl font-semibold tracking-tight text-foreground">
          SoundGrammy
        </h1>

        <SidebarProfile trackCount={trackCount} />
      </div>

      <Separator />

      <SidebarPlaylists />
    </div>
  );
}
