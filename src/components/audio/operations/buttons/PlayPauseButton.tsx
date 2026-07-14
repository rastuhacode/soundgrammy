import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { Pause, Play } from 'lucide-react'

export type PlayPauseButtonProps = {
  isPlaying: boolean
  onPlayToggle: () => void
}

export function PlayPauseButton(props: PlayPauseButtonProps) {
  return (
    <Button
      onClick={props.onPlayToggle}
      aria-label={props.isPlaying ? 'Pause' : 'Play'}
      className={
        cn(
          'flex items-center justify-center',
          'size-11 rounded-full bg-primary text-primary-foreground',
          'transition-transform hover:scale-105 active:scale-95 hover:bg-primary',
          '[&>svg]:fill-current [&>svg]:size-5',
        )
      }
    >
      { props.isPlaying ? <Pause /> : <Play /> }
    </Button>

  )
}
