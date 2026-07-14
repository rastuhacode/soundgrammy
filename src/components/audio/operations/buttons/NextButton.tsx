import { cn } from '@/lib/utils'
import { SkipForward } from 'lucide-react'

export type NextButtonProps = {
  onNextTrack: () => void
}

export function NextButton(props: NextButtonProps) {
  return (
    <button
      type="button"
      aria-label="Next track"
      className={
        cn(
          'flex items-center justify-center',
          'text-primary-foreground',
          'transition-transform hover:scale-105 active:scale-95',
          '[&>svg]:fill-current [&>svg]:size-5',
        )
      }
      onClick={props.onNextTrack}
    >
      <SkipForward />
    </button>
  )
}
