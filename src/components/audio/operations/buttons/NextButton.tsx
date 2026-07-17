import { SkipForward } from 'lucide-react'
import { cn } from '@/lib/utils'
import { usePlayerStore } from '@/stores/player-store'

export function NextButton() {
  const playNext = usePlayerStore(state => state.playNext)

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
      onClick={() => playNext()}
    >
      <SkipForward />
    </button>
  )
}
