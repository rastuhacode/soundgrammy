import {
  Pause,
  Play,
  SkipBack,
  SkipForward,
  Repeat,
  Repeat1,
  Shuffle,
} from 'lucide-react'
import type { RepeatState } from '@/lib/repeat'
import type { ShuffleState } from '@/lib/shuffle'
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
        className={
          cn(
            'flex items-center justify-center',
            'size-11 rounded-full bg-primary text-primary-foreground',
            'transition-transform hover:scale-105 active:scale-95',
            '[&>svg]:fill-current [&>svg]:size-5',
          )
        }
      >
        { props.isPlaying ? <Pause /> : <Play /> }
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
