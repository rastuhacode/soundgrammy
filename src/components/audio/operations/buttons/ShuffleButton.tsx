import { Shuffle } from 'lucide-react'
import { cn } from '@/lib/utils'
import { usePlayerStore } from '@/stores/player-store'
import { useShuffleStore } from '@/stores/shuffle-store'

export function ShuffleButton() {
  const shuffleState = useShuffleStore(state => state.shuffle)
  const toggleShuffle = usePlayerStore(state => state.toggleShuffle)

  return (
    <button
      type="button"
      aria-label="Shuffle mode"
      className={
        cn(
          'flex items-center justify-center transition-transform hover:scale-105 active:scale-95 [&>svg]:size-4',
          shuffleState === 'on'
            ? 'text-primary'
            : 'text-muted-foreground',
        )
      }
      onClick={toggleShuffle}
    >
      <Shuffle />
    </button>
  )
}
