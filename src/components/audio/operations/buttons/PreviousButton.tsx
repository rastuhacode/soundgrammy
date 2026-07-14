import { cn } from '@/lib/utils'
import { SkipBack } from 'lucide-react'

export type PreviousButtonProps = {
  onPreviousTrack: () => void
}

export function PreviousButton(props: PreviousButtonProps) {
  return (
    <button
      type="button"
      aria-label="Previous track"
      className={
        cn(
          'flex items-center justify-center',
          'text-primary-foreground',
          'transition-transform hover:scale-105 active:scale-95',
          '[&>svg]:fill-current [&>svg]:size-5',
        )
      }
      onClick={props.onPreviousTrack}
    >
      <SkipBack />
    </button>
  )
}
