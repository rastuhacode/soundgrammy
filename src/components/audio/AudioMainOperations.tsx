import {
  Pause,
  Play,
  SkipBack,
  SkipForward,
  Repeat,
  Repeat1,
  Shuffle,
} from 'lucide-react'
import type { RepeatState } from '@/stores/repeat-store'
import type { ShuffleState } from '@/stores/shuffle-store'
import { cn } from '@/lib/utils'

type AudioMainOperationsProps = {
  isPlaying: boolean
  onPlayToggle: () => void

  onPreviousTrack: () => void
  onNextTrack: () => void

  repeatState: RepeatState
  onRepeatToggle: () => void

  shuffleState: ShuffleState
  onShuffleToggle: () => void
}

export function AudioMainOperations(props: AudioMainOperationsProps) {
  return (
    <div className="flex items-center gap-6">
      <div className="flex items-center gap-4">
        <button
          type="button"
          onClick={props.onRepeatToggle}
          aria-label="Toggle repeat"
          className="text-primary"
        >
          {props.repeatState === 'none' || props.repeatState === 'all'
            ? (
                <Repeat
                  className={cn(
                    'size-4',
                    props.repeatState === 'none' && 'text-muted-foreground',
                  )}
                />
              )
            : (
                <Repeat1 className="size-4" />
              )}
        </button>
        <button
          type="button"
          onClick={props.onPreviousTrack}
          aria-label="Previous track"
        >
          <SkipBack className="size-5" />
        </button>
      </div>

      <button
        type="button"
        onClick={props.onPlayToggle}
        aria-label={props.isPlaying ? 'Pause' : 'Play'}
        className="flex size-11 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-[0_0_24px_-4px_var(--primary)] transition-transform hover:scale-105 active:scale-95"
      >
        {props.isPlaying
          ? (
              <Pause className="size-5 fill-current" />
            )
          : (
              <Play className="size-5 translate-x-px fill-current" />
            )}
      </button>

      <div className="flex items-center gap-4">
        <button type="button" onClick={props.onNextTrack} aria-label="Next track">
          <SkipForward className="size-5" />
        </button>
        <button
          type="button"
          aria-label="Shuffle mode"
          className={
            props.shuffleState === 'on'
              ? 'text-primary'
              : 'text-muted-foreground'
          }
          onClick={props.onShuffleToggle}
        >
          <Shuffle className="size-4" />
        </button>
      </div>
    </div>
  )
}
