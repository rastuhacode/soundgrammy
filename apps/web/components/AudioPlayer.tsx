"use client";

import { useRef, useState, useEffect, useLayoutEffect } from "react";
import type { Track } from "../lib/db";
import { Pause, Play, Volume2, Music } from "lucide-react";

const VOLUME_STORAGE_KEY = "soundgrammy-volume";

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds)) return "0:00";
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

function getStoredVolume(): number {
  if (typeof window === "undefined") return 100;
  const stored = localStorage.getItem(VOLUME_STORAGE_KEY);
  if (stored === null) return 100;
  const value = Number(stored);
  return Number.isFinite(value) && value >= 0 && value <= 100 ? value : 100;
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
  const [volume, setVolume] = useState(100);
  const [thumbError, setThumbError] = useState(false);
  const audioRef = useRef<HTMLAudioElement>(null);
  const volumeRef = useRef(volume);
  const isSeekingRef = useRef(false);

  volumeRef.current = volume;

  const applyVolume = () => {
    const audio = audioRef.current;
    if (!audio) return;
    audio.volume = volumeRef.current / 100;
  };

  const setAudioRef = (node: HTMLAudioElement | null) => {
    audioRef.current = node;
    if (node) {
      node.volume = volumeRef.current / 100;
    }
  };

  const togglePlay = () => {
    if (!audioRef.current || !track) return;
    if (isPlaying) {
      audioRef.current.pause();
    } else {
      audioRef.current.play().catch(() => onPlayingChange(false));
    }
  };

  const handleSeek = (e: React.FormEvent<HTMLInputElement>) => {
    if (!audioRef.current) return;
    const time = Number(e.currentTarget.value);
    isSeekingRef.current = true;
    audioRef.current.currentTime = time;
    setCurrentTime(time);
  };

  const handleSeekEnd = () => {
    isSeekingRef.current = false;
  };

  const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const nextVolume = Number(e.target.value);
    setVolume(nextVolume);
    volumeRef.current = nextVolume;
    applyVolume();
    localStorage.setItem(VOLUME_STORAGE_KEY, String(nextVolume));
  };

  useLayoutEffect(() => {
    const storedVolume = getStoredVolume();
    volumeRef.current = storedVolume;
    setVolume(storedVolume);
    applyVolume();
  }, []);

  useEffect(() => {
    applyVolume();
  }, [volume]);

  useEffect(() => {
    setThumbError(false);
  }, [track?.id]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !track) return;

    audio.src = `/api/tracks/${track.id}/stream`;
    applyVolume();
    audio.load();

    if (isPlaying) {
      audio.play().catch(() => {
        onPlayingChange(false);
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only reload audio when track changes
  }, [track?.id]);

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
    if (isSeekingRef.current) return;
    setCurrentTime(event.target.currentTime);
  };

  const onDurationChange = (event: React.ChangeEvent<HTMLAudioElement>) => {
    setDuration(event.target.duration);
  };

  const progress = duration > 0 ? (currentTime / duration) * 100 : 0;

  return (
    <>
      <audio
        ref={setAudioRef}
        preload="metadata"
        onPlay={play}
        onPause={pause}
        onTimeUpdate={onTimeUpdate}
        onDurationChange={onDurationChange}
        onEnded={onEnd}
        className="hidden"
      />

      {track ? (
        <div className="fixed bottom-0 left-0 right-0 flex items-center gap-4 px-6 py-4 bg-background border-t border-border h-30">
          <div className="flex gap-4 items-center min-w-40 w-1/5">
            {thumbError ? (
              <div className="rounded-md w-20 h-20 shrink-0 bg-muted flex items-center justify-center aspect-square">
                <Music className="opacity-60" />
              </div>
            ) : (
              <img
                src={`/api/tracks/${track.id}/thumbnail`}
                alt="thumbnail"
                className="rounded-md w-20 h-20 object-cover aspect-square shrink-0"
                onError={() => setThumbError(true)}
              />
            )}
            <div className="flex flex-col gap-1 grow max-w-full overflow-hidden">
              <span
                className="text-sm font-medium overflow-hidden text-ellipsis whitespace-nowrap"
                title={track.title ?? "Unknown Title"}
              >
                {track.title ?? "Unknown Title"}
              </span>
              <span
                className="text-sm opacity-60 overflow-hidden text-ellipsis whitespace-nowrap"
                title={track.performer ?? "Unknown Artist"}
              >
                {track.performer ?? "Unknown Artist"}
              </span>
            </div>
          </div>

          <div className="flex flex-col items-center gap-2 grow min-w-0">
            <div className="flex items-center gap-2 w-full">
              <span className="text-sm opacity-60 font-mono shrink-0 w-8 text-center">
                {formatTime(currentTime)}
              </span>
              <input
                type="range"
                className="w-full h-1 appearance-none rounded-[2px] outline-none cursor-pointer bg-primary [&::-webkit-slider-thumb]:bg-white"
                min={0}
                max={duration || 0}
                step={0.1}
                value={currentTime}
                onInput={handleSeek}
                onChange={handleSeek}
                onMouseUp={handleSeekEnd}
                onTouchEnd={handleSeekEnd}
                style={{ "--progress": `${progress}%` } as React.CSSProperties}
              />
              <span className="text-sm opacity-60 font-mono shrink-0 w-8 text-center">
                {formatTime(duration)}
              </span>
            </div>

            <button
              className="w-5 h-5 rounded-full bg-transparent text-foreground text-sm cursor-pointer flex items-center justify-center"
              onClick={togglePlay}
              aria-label={isPlaying ? "Pause" : "Play"}
            >
              {isPlaying ? <Pause /> : <Play />}
            </button>
          </div>

          <div className="flex items-center gap-2 min-w-40 w-1/5">
            <span className="text-sm opacity-60 font-mono shrink-0 w-8 text-center">
              <Volume2 />
            </span>
            <input
              type="range"
              className="w-full h-1 appearance-none rounded-[2px] outline-none cursor-pointer bg-primary [&::-webkit-slider-thumb]:bg-white"
              min={0}
              max={100}
              step={0.01}
              value={volume}
              onChange={handleVolumeChange}
              autoComplete="off"
            />
          </div>
        </div>
      ) : null}
    </>
  );
}
