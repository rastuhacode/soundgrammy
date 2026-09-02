import { SkipForward } from 'lucide-react'
import { cn } from '@/lib/utils'
import { usePlayerStore } from '@/stores/player-store'

export function NextButton() {
  const playNext = usePlayerStore(state => state.playNext)

  const title = 'Next track'

  return (
    <button
      type="button"
      aria-label={title}
      title={title}
      className={
        cn(
          'flex items-center justify-center',
          'text-foreground',
          'transition-transform hover:scale-105 active:scale-95',
          '[&>svg]:fill-foreground [&>svg]:size-5',
        )
      }
      onClick={() => playNext()}
    >
      <SkipForward />
    </button>
  )
}
