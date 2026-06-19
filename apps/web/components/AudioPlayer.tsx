"use client";

import { useRef, useState, useEffect, useLayoutEffect } from "react";
import { Pause, Play, Volume2, Music } from "lucide-react";
import { AudioProgressBar } from "@/components/audio/AudioProgressBar";
import { usePlayerStore } from "@/stores/player-store";

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

export function AudioPlayer() {
  const track = usePlayerStore((state) => state.currentTrack);
  const isPlaying = usePlayerStore((state) => state.isPlaying);
  const setPlaying = usePlayerStore((state) => state.setPlaying);
  const playNext = usePlayerStore((state) => state.playNext);

  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [bufferedTime, setBufferedTime] = useState(0);
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
      audioRef.current.play().catch(() => setPlaying(false));
    }
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
        setPlaying(false);
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only reload audio when track changes
  }, [track?.id]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !track) return;

    if (isPlaying) {
      audio.play().catch(() => setPlaying(false));
    } else {
      audio.pause();
    }
  }, [isPlaying, track, setPlaying]);

  const onTimeUpdate = (event: React.ChangeEvent<HTMLAudioElement>) => {
    if (isSeekingRef.current) return;
    setCurrentTime(event.target.currentTime);
  };

  const onDurationChange = (event: React.ChangeEvent<HTMLAudioElement>) => {
    setDuration(event.target.duration);
  };

  const onProgress = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    const audio = event.currentTarget;
    if (!audio.buffered.length) {
      setBufferedTime(0);
      return;
    }
    setBufferedTime(audio.buffered.end(audio.buffered.length - 1));
  };

  return (
    <>
      <audio
        ref={setAudioRef}
        preload="metadata"
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onTimeUpdate={onTimeUpdate}
        onDurationChange={onDurationChange}
        onProgress={onProgress}
        onEnded={playNext}
        aria-hidden={true}
        className="hidden"
      />

      {track ? (
        <div className="relative flex h-24 w-full flex-col border-t border-border bg-card/80 backdrop-blur-xl">
          <AudioProgressBar
            currentTime={currentTime}
            duration={duration}
            bufferedTime={bufferedTime}
            onSeek={(time) => {
              if (!audioRef.current) return;
              isSeekingRef.current = true;
              audioRef.current.currentTime = time;
              setCurrentTime(time);
            }}
            onSeekStart={() => {
              isSeekingRef.current = true;
            }}
            onSeekEnd={handleSeekEnd}
          />

          <div className="flex h-full w-full items-center gap-2 px-4">
            <div className="flex w-1/3 items-center gap-3">
              <div className="relative shrink-0">
                {thumbError ? (
                  <div className="flex size-14 items-center justify-center rounded-lg bg-muted text-muted-foreground">
                    <Music className="size-5" />
                  </div>
                ) : (
                  <img
                    src={`/api/tracks/${track.id}/thumbnail`}
                    alt="Thumbnail"
                    className="size-16 rounded-lg object-cover ring-1 ring-border"
                    onError={() => setThumbError(true)}
                  />
                )}
              </div>
              <div className="hidden min-w-0 flex-col sm:flex">
                <span
                  className="truncate text-sm font-medium text-foreground"
                  title={track.title ?? "Unknown Title"}
                >
                  {track.title ?? "Unknown Title"}
                </span>
                <span
                  className="truncate text-xs text-muted-foreground"
                  title={track.performer ?? "Unknown Artist"}
                >
                  {track.performer ?? "Unknown Artist"}
                </span>
              </div>
            </div>

            <div className="flex w-1/3 flex-col items-center gap-2">
              <button
                type="button"
                onClick={togglePlay}
                aria-label={isPlaying ? "Pause" : "Play"}
                className="flex size-11 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-[0_0_24px_-4px_var(--primary)] transition-transform hover:scale-105 active:scale-95"
              >
                {isPlaying ? (
                  <Pause className="size-5 fill-current" />
                ) : (
                  <Play className="size-5 translate-x-px fill-current" />
                )}
              </button>
              <div className="font-mono text-xs tabular-nums text-muted-foreground">
                {formatTime(currentTime)}
                <span className="mx-1 text-muted-foreground/60">/</span>
                {formatTime(duration)}
              </div>
            </div>

            <div className="w-1/3 items-center gap-2 flex justify-end">
              <Volume2 className="size-4 shrink-0 text-muted-foreground" />
              <input
                type="range"
                className="hifi-range h-1 w-32 appearance-none rounded-full outline-none"
                min={0}
                max={100}
                step={0.01}
                value={volume}
                onChange={handleVolumeChange}
                autoComplete="off"
                aria-label="Volume"
                style={{ "--progress": `${volume}%` } as React.CSSProperties}
              />
            </div>
          </div>
        </div>
      ) : (
        <></>
      )}
    </>
  );
}
