import { Repeat, Repeat1 } from 'lucide-react'
import { usePlayerStore } from '@/stores/player-store'
import { useRepeatStore } from '@/stores/repeat-store'

export function RepeatButton() {
  const repeatState = useRepeatStore(state => state.repeat)
  const toggleRepeat = usePlayerStore(state => state.toggleRepeat)

  const title = 'Toggle repeat'

  return (
    <button
      type="button"
      aria-label={title}
      title={title}
      className="flex items-center justify-center text-primary transition-transform hover:scale-105 active:scale-95 [&>svg]:size-4"
      onClick={toggleRepeat}
    >
      {repeatState === 'none' || repeatState === 'all'
        ? (
            <Repeat
              className={repeatState === 'none' ? 'text-muted-foreground' : ''}
            />
          )
        : (
            <Repeat1 />
          )}
    </button>
  )
}
