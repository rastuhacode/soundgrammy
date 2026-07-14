import { SkipBack } from 'lucide-react'
import { previousOrRestart } from '@/lib/playback-controller'
import { cn } from '@/lib/utils'

export function PreviousButton() {
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
      onClick={previousOrRestart}
    >
      <SkipBack />
    </button>
  )
}
