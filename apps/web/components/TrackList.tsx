"use client";

import type { Track } from "../lib/db";
import styles from "./TrackList.module.css";

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
}

export function TrackList({ tracks, currentTrackId, isPlaying, onTrackSelect }: TrackListProps) {
  if (tracks.length === 0) {
    return (
      <div className={styles.empty}>
        <p>No tracks yet</p>
        <p className={styles.emptyHint}>Send audio files to the bot in Telegram to add music</p>
      </div>
    );
  }

  return (
    <ul className={styles.list}>
      {tracks.map((track) => {
        const isActive = currentTrackId === track.id;
        return (
          <li key={track.id}>
            <button
              className={`${styles.track} ${isActive ? styles.active : ""}`}
              onClick={() => onTrackSelect(track)}
            >
              <div className={styles.playIndicator}>
                {isActive && isPlaying ? (
                  <span className={styles.equalizer}>
                    <span /><span /><span />
                  </span>
                ) : (
                  <span className={styles.playIcon}>&#9654;</span>
                )}
              </div>
              <div className={styles.info}>
                <span className={styles.title}>{track.title ?? "Unknown Title"}</span>
                <span className={styles.performer}>{track.performer ?? "Unknown Artist"}</span>
              </div>
              <span className={styles.duration}>{formatDuration(track.duration)}</span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
