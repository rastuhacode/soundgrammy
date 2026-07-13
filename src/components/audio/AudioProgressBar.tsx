import { useState } from 'react'
import { cn } from '@/lib/utils'

export interface AudioBufferedRange {
  start: number
  end: number
}

export interface AudioProgressBarProps {
  currentTime: number
  duration: number
  bufferedRanges?: AudioBufferedRange[]
  showInitialLoading?: boolean
  onSeek: (time: number) => void
  onSeekStart?: () => void
  onSeekEnd?: () => void
  className?: string
}

export function AudioProgressBar({
  currentTime,
  duration,
  bufferedRanges = [],
  showInitialLoading = false,
  onSeek,
  onSeekStart,
  onSeekEnd,
  className,
}: AudioProgressBarProps) {
  const [isDragging, setIsDragging] = useState(false)

  const progress
    = duration > 0 ? Math.min(100, (currentTime / duration) * 100) : 0
  const handleSeek = (e: React.FormEvent<HTMLInputElement>) => {
    onSeek(Number(e.currentTarget.value))
  }

  const handleSeekStart = () => {
    setIsDragging(true)
    onSeekStart?.()
  }

  const handleSeekEnd = () => {
    setIsDragging(false)
    onSeekEnd?.()
  }

  return (
    <div
      className={cn(
        'group/audiobar absolute h-8 -top-4 w-full z-100',
        className,
      )}
    >
      <div
        aria-hidden
        className="pointer-events-none absolute inset-x-0 top-4 h-1 overflow-hidden bg-foreground/15"
      >
        {showInitialLoading && (
          <div className="barbershop-buffer absolute inset-0 opacity-35" />
        )}
        {bufferedRanges.map((range, index) => {
          const start = duration > 0
            ? Math.max(0, Math.min(100, (range.start / duration) * 100))
            : 0
          const end = duration > 0
            ? Math.max(start, Math.min(100, (range.end / duration) * 100))
            : 0
          return (
            <div
              key={`${index}:${range.start}:${range.end}`}
              className="absolute inset-y-0 bg-foreground/30"
              style={{ left: `${start}%`, width: `${end - start}%` }}
            />
          )
        })}
        <div
          className="absolute inset-y-0 left-0 bg-primary transition-all duration-200"
          style={{ width: `${progress}%` }}
        />
      </div>

      <div
        aria-hidden
        className={cn(
          'pointer-events-none absolute top-[18px] z-10 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-primary opacity-0 transition-all duration-200 group-hover/audiobar:opacity-100',
          isDragging && 'scale-125 opacity-100',
        )}
        style={{ left: `${progress}%` }}
      />

      <input
        type="range"
        className="absolute inset-0 h-full w-full appearance-none opacity-0 [&::-moz-range-thumb]:h-4 [&::-moz-range-thumb]:w-0 [&::-moz-range-thumb]:border-0 [&::-moz-range-thumb]:bg-transparent [&::-moz-range-track]:bg-transparent [&::-webkit-slider-runnable-track]:bg-transparent [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-0 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:bg-transparent"
        min={0}
        max={duration || 0}
        step={0.1}
        value={currentTime}
        onInput={handleSeek}
        onPointerDown={handleSeekStart}
        onPointerUp={handleSeekEnd}
        onPointerCancel={handleSeekEnd}
        onTouchEnd={handleSeekEnd}
        onKeyDown={handleSeekStart}
        onKeyUp={handleSeekEnd}
        onBlur={handleSeekEnd}
        aria-label="Seek"
        aria-valuemin={0}
        aria-valuemax={duration || 0}
        aria-valuenow={currentTime}
      />
    </div>
  )
}
