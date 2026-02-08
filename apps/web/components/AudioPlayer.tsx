"use client";

import { useRef, useState, useEffect, useCallback } from "react";
import type { Track } from "../lib/db";
import styles from "./AudioPlayer.module.css";

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds)) return "0:00";
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

interface AudioPlayerProps {
  track: Track | null;
  isPlaying: boolean;
  onPlayingChange: (playing: boolean) => void;
  onEnd: () => void;
}

export function AudioPlayer({
  track,
  isPlaying,
  onPlayingChange,
  onEnd,
}: AudioPlayerProps) {
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);

  const audioRef = useRef<HTMLAudioElement>(null);

  const togglePlay = () => {
    if (!audioRef.current || !track) return;
    if (isPlaying) {
      audioRef.current.pause();
    } else {
      audioRef.current.play().catch(() => onPlayingChange(false));
    }
  };

  const handleSeek = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (!audioRef.current) return;
    const time = Number(e.target.value);
    audioRef.current.currentTime = time;
    setCurrentTime(time);
  };

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !track) return;

    audio.src = `/api/tracks/${track.file_id}/stream`;
    audio.load();

    if (isPlaying) {
      audio.play().catch(() => {
        onPlayingChange(false);
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only reload audio when track changes
  }, [track?.file_id]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !track) return;

    if (isPlaying) {
      audio.play().catch(() => onPlayingChange(false));
    } else {
      audio.pause();
    }
  }, [isPlaying, track, onPlayingChange]);

  const play = () => onPlayingChange(true);
  const pause = () => onPlayingChange(false);

  const onTimeUpdate = (event: React.ChangeEvent<HTMLAudioElement>) => {
    setCurrentTime(event.target.currentTime);
  };

  const onDurationChange = (event: React.ChangeEvent<HTMLAudioElement>) => {
    setDuration(event.target.duration);
  };

  if (!track) return null;

  const progress = duration > 0 ? (currentTime / duration) * 100 : 0;

  return (
    <div className={styles.player}>
      <audio
        ref={audioRef}
        preload="metadata"
        onPlay={play}
        onPause={pause}
        onTimeUpdate={onTimeUpdate}
        onDurationChange={onDurationChange}
        onEnded={onEnd}
      />

      <div className={styles.trackInfo}>
        <span className={styles.title}>{track.title ?? "Unknown Title"}</span>
        <span className={styles.performer}>
          {track.performer ?? "Unknown Artist"}
        </span>
      </div>

      <div className={styles.controls}>
        <button
          className={styles.playButton}
          onClick={togglePlay}
          aria-label={isPlaying ? "Pause" : "Play"}
        >
          {isPlaying ? "⏸" : "▶"}
        </button>
      </div>

      <div className={styles.progress}>
        <span className={styles.time}>{formatTime(currentTime)}</span>
        <div className={styles.seekBarContainer}>
          <input
            type="range"
            className={styles.seekBar}
            min={0}
            max={duration || 0}
            step={0.1}
            value={currentTime}
            onChange={handleSeek}
            style={{ "--progress": `${progress}%` } as React.CSSProperties}
          />
        </div>
        <span className={styles.time}>{formatTime(duration)}</span>
      </div>
    </div>
  );
}
