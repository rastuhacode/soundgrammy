"use client";

import type { Track } from "@/lib/db";
import { Play, Trash } from "lucide-react";

function formatDuration(seconds: number | null): string {
  if (seconds === null) return "--:--";
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

interface TrackListProps {
  tracks: Track[];
  currentTrackId: number | null;
  isPlaying: boolean;
  onTrackSelect: (track: Track) => void;
  onDelete: (track: Track) => void;
}

export function TrackList({
  tracks,
  currentTrackId,
  isPlaying,
  onTrackSelect,
  onDelete,
}: TrackListProps) {
  if (tracks.length === 0) {
    return (
      <div className="flex flex-col gap-2 justify-center items-center p-4">
        <p>No tracks yet</p>
        <p className="text-sm opacity-60">
          No music pinned to your Telegram profile yet
        </p>
      </div>
    );
  }

  return (
    <ul className="list-none flex flex-col gap-2">
      {tracks.map((track) => {
        const isActive = currentTrackId === track.id;
        return (
          <li key={track.id}>
            <div
              className={`flex items-center gap-4 w-full p-4 border-none bg-transparent text-foreground rounded-md transition-background duration-150 font-sans text-left ${isActive ? "bg-gray-100" : ""}`}
            >
              <button
                className="w-6 h-6 flex items-center justify-center shrink-0"
                onClick={() => onTrackSelect(track)}
              >
                {isActive && isPlaying ? (
                  <span className="flex items-end gap-1 h-4 equalizer">
                    <span className="w-[3px] bg-foreground rounded-[1px]" />
                    <span />
                    <span />
                  </span>
                ) : (
                  <Play />
                )}
              </button>
              <div className="flex flex-col gap-1 min-w-0 flex-1">
                <span className="text-sm font-medium overflow-hidden text-ellipsis whitespace-nowrap">
                  {track.title ?? "Unknown Title"}
                </span>
                <span className="text-sm opacity-60 overflow-hidden text-ellipsis whitespace-nowrap">
                  {track.performer ?? "Unknown Artist"}
                </span>
              </div>
              <span className="text-sm opacity-50 font-mono shrink-0">
                {formatDuration(track.duration)}
              </span>
              <button aria-label="Delete track" onClick={() => onDelete(track)}>
                <Trash />
              </button>
            </div>
          </li>
        );
      })}
    </ul>
  );
}
