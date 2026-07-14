import type { RepeatState } from '@/lib/repeat'
import { Repeat, Repeat1 } from 'lucide-react'

export type RepeatButtonProps = {
  repeatState: RepeatState
  onRepeatToggle: () => void
}

export function RepeatButton(props: RepeatButtonProps) {
  return (
    <button
      type="button"
      aria-label="Toggle repeat"
      className="flex items-center justify-center text-primary transition-transform hover:scale-105 active:scale-95 [&>svg]:size-4"
      onClick={props.onRepeatToggle}
    >
      {props.repeatState === 'none' || props.repeatState === 'all'
        ? (
            <Repeat
              className={props.repeatState === 'none' ? 'text-muted-foreground' : ''}
            />
          )
        : (
            <Repeat1 />
          )}

    </button>
  )
}
