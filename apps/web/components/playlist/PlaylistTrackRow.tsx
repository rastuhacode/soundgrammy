"use client";

import type { Track } from "@/lib/db";
import type { ResolvedSelectedPlaylist } from "@/stores/playlists-store";
import { cn } from "@/lib/utils";
import { TrackThumbnail } from "./PlaylistTrackThumbnail";
import { PlaylistTrackDropdown } from "./PlaylistTrackDropdown";
import { Play } from "lucide-react";

export const TRACK_ROW_HEIGHT = 70; // 70px

export interface TrackRowProps {
  track: Track;
  isActive: boolean;
  isPlaying: boolean;
  isLiked: boolean;
  currentPlaylist: ResolvedSelectedPlaylist;
  customPlaylists: { id: number; name: string; trackIds: number[] }[];
  className?: string;
  style?: React.CSSProperties;
  onTrackSelect: (track: Track) => void;
  onToggleLike: (trackId: number) => void;
  onAddToPlaylist: (playlistId: number, trackId: number) => void;
  onDeleteFromPlaylist: (playlistId: number, trackId: number) => void;
  onDownload: (track: Track) => void;
  onShowInfo: (track: Track) => void;
}

export function PlaylistTrackRow({
  track,
  isActive,
  isPlaying,
  isLiked,
  currentPlaylist,
  customPlaylists,
  className,
  style,
  onTrackSelect,
  onToggleLike,
  onAddToPlaylist,
  onDeleteFromPlaylist,
  onDownload,
  onShowInfo,
}: TrackRowProps) {
  const showEqualizer = isActive && isPlaying;
  const availablePlaylists = customPlaylists.filter(
    (playlist) => !playlist.trackIds.includes(track.id),
  );

  function formatDuration(seconds: number | null): string {
    if (seconds === null) return "--:--";
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  }

  return (
    <li
      role="button"
      tabIndex={0}
      onClick={() => onTrackSelect(track)}
      aria-label={showEqualizer ? "Pause track" : "Play track"}
      className={cn("group relative flex items-center gap-3 rounded-lg px-2.5 transition-colors duration-200 cursor-pointer hover:bg-card/70", className)}
      style={{ height: `${TRACK_ROW_HEIGHT}px`, ...style }}
    >
      <div className="relative shrink-0">
        <TrackThumbnail
          trackId={track.id}
          fileUniqueId={track.file_unique_id}
        />
        <span
          className={`absolute inset-0 flex items-center justify-center rounded-sm bg-background/65 backdrop-blur-[1px] transition-opacity duration-200 ${
            isActive ? "opacity-100" : "opacity-0 group-hover:opacity-100"
          }`}
        >
          {showEqualizer
            ? (
                <span className="equalizer flex h-4 items-end gap-1">
                  <span className="w-[3px] rounded-[1px] bg-foreground" />
                  <span />
                  <span />
                </span>
              )
            : (
                <Play className={cn("size-5", isActive ? "fill-primary text-primary" : "fill-foreground text-foreground")} />
              )}
        </span>
      </div>

      <div
        className="flex min-w-0 grow flex-col items-start text-left"
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
      </div>

      <div className="flex shrink-0 items-center gap-2">
        <span className="w-10 shrink-0 text-right font-mono text-xs text-muted-foreground">
          {formatDuration(track.duration)}
        </span>

        <PlaylistTrackDropdown
          availablePlaylists={availablePlaylists}
          currentPlaylist={currentPlaylist}
          onAddToPlaylist={onAddToPlaylist}
          onToggleLike={onToggleLike}
          onDeleteFromPlaylist={onDeleteFromPlaylist}
          onDownload={onDownload}
          onShowInfo={onShowInfo}
          isLiked={isLiked}
          track={track}
        />
      </div>
    </li>
  );
}
