import type { ShuffleState } from '@/lib/shuffle'
import { cn } from '@/lib/utils'
import { Shuffle } from 'lucide-react'

export type ShuffleButtonProps = {
  shuffleState: ShuffleState
  onShuffleToggle: () => void
}

export function ShuffleButton(props: ShuffleButtonProps) {
  return (
    <button
      type="button"
      aria-label="Shuffle mode"
      className={
        cn(
          'flex items-center justify-center transition-transform hover:scale-105 active:scale-95 [&>svg]:size-4',
          props.shuffleState === 'on'
            ? 'text-primary'
            : 'text-muted-foreground',
        )
      }
      onClick={props.onShuffleToggle}
    >
      <Shuffle />
    </button>
  )
}
