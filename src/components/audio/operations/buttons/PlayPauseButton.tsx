import { Pause, Play } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { usePlayerStore } from '@/stores/player-store'

export function PlayPauseButton() {
  const isPlaying = usePlayerStore(state => state.isPlaying)
  const setPlaying = usePlayerStore(state => state.setPlaying)
  const track = usePlayerStore(state => state.currentTrack)

  const title = isPlaying ? 'Pause' : 'Play'

  function handleToggle() {
    if (!track) return
    setPlaying(!isPlaying)
  }

  return (
    <Button
      onClick={handleToggle}
      aria-label={title}
      title={title}
      className={
        cn(
          'flex items-center justify-center',
          'size-11 rounded-full bg-primary text-foreground',
          'transition-transform hover:scale-105 active:scale-95 hover:bg-primary',
          '[&>svg]:fill-foreground [&>svg]:size-5',
        )
      }
    >
      {isPlaying ? <Pause /> : <Play />}
    </Button>
  )
}
