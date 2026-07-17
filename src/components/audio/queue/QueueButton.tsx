import { ListMusic } from 'lucide-react'
import { useState } from 'react'
import type { PopoverRoot } from '@base-ui/react/popover'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
  type PopoverClasses,
} from '@/components/ui/popover'
import { usePlayerStore } from '@/stores/player-store'
import { QueuePopoverPanel } from './QueuePopoverPanel'

export interface QueueButtonProps {
  className?: string
  classes?: PopoverClasses
}

export function QueueButton({ className, classes }: QueueButtonProps) {
  const [open, setOpen] = useState(false)
  const trackCount = usePlayerStore(state => state.queue.tracks.length)

  function handleOpenChange(
    nextOpen: boolean,
    details: PopoverRoot.ChangeEventDetails,
  ) {
    if (details.reason === 'trigger-press') {
      setOpen(nextOpen)
      return
    }
    setOpen(nextOpen)
  }

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger
        render={(
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Queue"
            aria-expanded={open}
            className={cn(
              'transition-transform hover:scale-105 active:scale-95',
              className,
            )}
          >
            <ListMusic className="size-5 shrink-0" />
          </Button>
        )}
      />
      <PopoverContent
        side="top"
        sideOffset={41}
        className="w-80 p-0"
        classes={classes}
      >
        <QueuePopoverPanel
          onClose={() => setOpen(false)}
          hasTracks={trackCount > 0}
        />
      </PopoverContent>
    </Popover>
  )
}
