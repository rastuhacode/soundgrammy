"use client";

import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Music, Play } from "lucide-react";
import { useCachedThumbnail } from "@/hooks/use-cached-thumbnail";
import { usePlayerStore } from "@/stores/player-store";
import type { Track } from "@/lib/db";

const TRACK_ROW_HEIGHT = 70;

function formatDuration(seconds: number | null): string {
  if (seconds === null) return "--:--";
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

function TrackThumbnail({ trackId }: { trackId: number }) {
  const { url, loaded, failed } = useCachedThumbnail(trackId);

  return (
    <div className="relative size-12 shrink-0 overflow-hidden rounded-lg bg-muted">
      <div className="absolute inset-0 flex items-center justify-center text-muted-foreground">
        <Music className="size-5" />
      </div>
      {!failed && url && (
        <img
          src={url}
          alt=""
          decoding="async"
          className={`absolute inset-0 size-12 object-cover transition-opacity duration-200 ${
            loaded ? "opacity-100" : "opacity-0"
          }`}
        />
      )}
    </div>
  );
}

interface TrackRowProps {
  track: Track;
  isActive: boolean;
  isPlaying: boolean;
  onTrackSelect: (track: Track) => void;
}

function TrackRow({
  track,
  isActive,
  isPlaying,
  onTrackSelect,
}: TrackRowProps) {
  const showEqualizer = isActive && isPlaying;

  return (
    <div
      className={`group relative flex items-center gap-4 rounded-xl border px-3 py-2.5 transition-colors duration-200 ${
        isActive
          ? "border-primary/40 bg-primary/10"
          : "border-transparent hover:border-border hover:bg-card/70"
      }`}
    >
      <button
        type="button"
        onClick={() => onTrackSelect(track)}
        aria-label={showEqualizer ? "Pause track" : "Play track"}
        className="relative shrink-0"
      >
        <TrackThumbnail trackId={track.id} />
        <span
          className={`absolute inset-0 flex items-center justify-center rounded-lg bg-background/65 backdrop-blur-[1px] transition-opacity duration-200 ${
            isActive ? "opacity-100" : "opacity-0 group-hover:opacity-100"
          }`}
        >
          {showEqualizer ? (
            <span className="equalizer flex h-4 items-end gap-1">
              <span className="w-[3px] rounded-[1px] bg-foreground" />
              <span />
              <span />
            </span>
          ) : isActive ? (
            <Play className="size-5 fill-primary text-primary" />
          ) : (
            <Play className="size-5 fill-foreground text-foreground" />
          )}
        </span>
      </button>

      <button
        type="button"
        onClick={() => onTrackSelect(track)}
        className="flex min-w-0 flex-1 flex-col items-start text-left"
      >
        <span
          className={`max-w-full truncate text-sm font-medium ${
            isActive ? "text-primary" : "text-foreground"
          }`}
        >
          {track.title ?? "Unknown Title"}
        </span>
        <span className="max-w-full truncate text-sm text-muted-foreground">
          {track.performer ?? "Unknown Artist"}
        </span>
      </button>

      <span className="shrink-0 font-mono text-xs tabular-nums text-muted-foreground">
        {formatDuration(track.duration)}
      </span>
    </div>
  );
}

export function TrackList() {
  const tracks = usePlayerStore((state) => state.tracks);
  const currentTrackId = usePlayerStore(
    (state) => state.currentTrack?.id ?? null,
  );
  const isPlaying = usePlayerStore((state) => state.isPlaying);
  const selectTrack = usePlayerStore((state) => state.selectTrack);
  const scrollRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: tracks.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => TRACK_ROW_HEIGHT,
    overscan: 8,
  });

  if (tracks.length === 0) {
    return (
      <div className="animate-fade-up flex flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-border px-6 py-20 text-center">
        <div className="flex size-14 items-center justify-center rounded-full border border-border bg-card text-muted-foreground">
          <Music className="size-6" />
        </div>
        <p className="text-base font-medium text-foreground">No tracks yet</p>
        <p className="max-w-xs text-sm text-muted-foreground">
          Pin music to your Telegram profile and it will tune in here
          automatically.
        </p>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="mb-3 flex shrink-0 items-center gap-3 px-3">
        <h2 className="font-mono text-xs uppercase tracking-[0.28em] text-muted-foreground">
          Library
        </h2>
        <span className="dial-divider h-3 flex-1" />
      </div>
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto">
        <ul
          className="relative w-full list-none"
          style={{ height: `${virtualizer.getTotalSize()}px` }}
        >
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const track = tracks[virtualRow.index];
            if (!track) {
              return null;
            }

            return (
              <li
                key={track.id}
                className="absolute left-0 top-0 w-full px-0"
                style={{
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                <TrackRow
                  track={track}
                  isActive={currentTrackId === track.id}
                  isPlaying={isPlaying}
                  onTrackSelect={selectTrack}
                />
              </li>
            );
          })}
        </ul>
      </div>
    </div>
  );
}
