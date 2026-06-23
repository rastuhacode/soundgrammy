"use client";

import { useState } from "react";
import { cn } from "@/lib/utils";

export interface AudioProgressBarProps {
  currentTime: number;
  duration: number;
  bufferedTime?: number;
  onSeek: (time: number) => void;
  onSeekStart?: () => void;
  onSeekEnd?: () => void;
  className?: string;
}

export function AudioProgressBar({
  currentTime,
  duration,
  bufferedTime = 0,
  onSeek,
  onSeekStart,
  onSeekEnd,
  className,
}: AudioProgressBarProps) {
  const [isDragging, setIsDragging] = useState(false);

  const progress
    = duration > 0 ? Math.min(100, (currentTime / duration) * 100) : 0;
  const bufferedProgress
    = duration > 0 ? Math.min(100, (bufferedTime / duration) * 100) : 0;

  const handleSeek = (
    e: React.InputEvent<HTMLInputElement> | React.ChangeEvent<HTMLInputElement>,
  ) => {
    onSeek(Number(e.currentTarget.value));
  };

  const handleSeekStart = () => {
    setIsDragging(true);
    onSeekStart?.();
  };

  const handleSeekEnd = () => {
    setIsDragging(false);
    onSeekEnd?.();
  };

  return (
    <div
      className={cn(
        "group/audiobar absolute h-8 -top-4 w-full cursor-pointer z-100",
        className,
      )}
    >
      <div
        id="progressContainer"
        aria-hidden
        className="pointer-events-none absolute inset-x-0 top-4 h-1 bg-foreground/15"
      >
        <div
          className="absolute inset-y-0 left-0 bg-foreground/25"
          style={{ width: `${bufferedProgress}%` }}
        />
        <div
          id="primaryProgress"
          className="absolute inset-y-0 left-0 bg-primary transition-all duration-200"
          style={{ width: `${progress}%` }}
        />
        <div
          id="primaryProgressThumb"
          className={cn(
            "absolute top-1/2 size-0.5 -translate-y-1/2 rounded-full bg-primary opacity-0 transition-all duration-200 group-hover/audiobar:opacity-100 group-hover/audiobar:scale-700",
            isDragging && "scale-110",
          )}
          style={{ left: `calc(${progress}% - 5px)` }}
        />
      </div>

      <input
        type="range"
        className="absolute inset-0 h-full w-full cursor-pointer appearance-none opacity-0 [&::-moz-range-thumb]:size-4 [&::-moz-range-thumb]:border-0 [&::-moz-range-thumb]:bg-transparent [&::-moz-range-track]:bg-transparent [&::-webkit-slider-runnable-track]:bg-transparent [&::-webkit-slider-thumb]:size-4 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:bg-transparent"
        min={0}
        max={duration || 0}
        step={0.1}
        value={currentTime}
        onInput={handleSeek}
        onChange={handleSeek}
        onPointerDown={handleSeekStart}
        onPointerUp={handleSeekEnd}
        onTouchEnd={handleSeekEnd}
        aria-label="Seek"
        aria-valuemin={0}
        aria-valuemax={duration || 0}
        aria-valuenow={currentTime}
      />
    </div>
  );
}
