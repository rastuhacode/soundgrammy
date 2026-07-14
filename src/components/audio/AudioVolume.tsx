import { useState } from 'react'
import { Volume2, VolumeX } from 'lucide-react'
import type { PopoverRoot } from '@base-ui/react/popover'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
  type PopoverClasses,
} from '@/components/ui/popover'

export interface AudioVolumeProps {
  volume: number
  onVolumeChange: (e: React.ChangeEvent<HTMLInputElement>) => void
  onMuteToggle: () => void
  classes?: PopoverClasses
}

export function AudioVolume(props: AudioVolumeProps) {
  const [open, setOpen] = useState(false)
  const isMuted = props.volume === 0
  const VolumeIcon = isMuted ? VolumeX : Volume2

  function handleOpenChange(
    nextOpen: boolean,
    details: PopoverRoot.ChangeEventDetails,
  ) {
    if (details.reason === 'trigger-press') return
    setOpen(nextOpen)
  }

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger
        openOnHover
        delay={0}
        closeDelay={150}
        render={(
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={isMuted ? 'Unmute' : 'Mute'}
            onClick={props.onMuteToggle}
          >
            <VolumeIcon className="size-5 shrink-0" />
          </Button>
        )}
      />
      <PopoverContent
        side="top"
        align="center"
        className="w-fit px-1"
        classes={props.classes}
      >
        <div className="flex h-24 w-8 items-center justify-center">
          <input
            type="range"
            className={
              cn(
                'appearance-none outline-none w-24 h-0.5 -rotate-90',
                '[background:linear-gradient(to_right,var(--primary)_0%,var(--primary)_var(--progress,0%),color-mix(in_oklch,var(--foreground)_18%,transparent)_var(--progress,0%),color-mix(in_oklch,var(--foreground)_18%,transparent)_100%)]',
                '[&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-primary [&::-webkit-slider-thumb]:appearance-none',
                '[&::-webkit-slider-thumb]:transition-transform [&::-webkit-slider-thumb]:duration-150 [&::-webkit-slider-thumb:hover]:scale-125',
                '[&::-moz-range-thumb]:size-2 [&::-webkit-slider-thumb]:size-2',
                '[&::-moz-range-thumb]:border-none [&::-moz-range-thumb]:bg-primary [&::-moz-range-thumb]:rounded-full',
              )
            }
            min={0}
            max={100}
            step={0.01}
            value={props.volume}
            onChange={props.onVolumeChange}
            autoComplete="off"
            aria-label="Volume"
            style={{ '--progress': `${props.volume}%` } as React.CSSProperties}
          />
        </div>
      </PopoverContent>
    </Popover>
  )
}
