import { useRef, useState, type ReactNode } from 'react'
import { Check, CircleHelp, Shuffle } from 'lucide-react'
import { cn } from '@/lib/utils'
import { SHUFFLE_MODE_OPTIONS, type ShuffleMode } from '@/lib/shuffle'
import { usePlayerStore } from '@/stores/player-store'
import { useShuffleStore } from '@/stores/shuffle-store'
import { Popover, PopoverContent } from '@/components/ui/popover'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'

type ShufflePopoverItemProps = {
  children?: ReactNode
  isActive?: boolean
  onClick?: () => void
}

function ShufflePopoverItem(props: ShufflePopoverItemProps) {
  return (
    <button
      type="button"
      aria-pressed={props.isActive}
      className={cn(
        'flex w-full gap-1 items-center justify-between rounded-md px-2 py-2 text-left hover:bg-accent',
        props.isActive && 'bg-accent text-accent-foreground',
      )}
      onClick={props.onClick}
    >
      <span className="font-medium">{props.children}</span>
      {props.isActive && <Check className="size-3 text-primary" />}
    </button>
  )
}

export function ShuffleButton() {
  const [open, setOpen] = useState(false)
  const buttonRef = useRef<HTMLButtonElement>(null)
  const shuffleState = useShuffleStore(state => state.shuffle)
  const shuffleMode = useShuffleStore(state => state.mode)
  const setShuffle = usePlayerStore(state => state.setShuffle)
  const setShuffleMode = usePlayerStore(state => state.setShuffleMode)
  const activeLabel = SHUFFLE_MODE_OPTIONS.find(
    option => option.id === shuffleMode,
  )?.label ?? 'Random'

  const selectMode = (mode: ShuffleMode) => {
    setShuffleMode(mode)
    setOpen(false)
  }

  const toggleShuffle = () => {
    setShuffle(shuffleState === 'on' ? 'off' : 'on')
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <button
        ref={buttonRef}
        type="button"
        aria-label="Shuffle"
        aria-pressed={shuffleState === 'on'}
        aria-keyshortcuts="Shift+F10"
        title={`Shuffle: ${shuffleState === 'on' ? activeLabel : 'Off'}. Right-click to choose mode.`}
        className={cn(
          'flex items-center justify-center transition-transform hover:scale-105 active:scale-95 [&>svg]:size-4',
          shuffleState === 'on'
            ? 'text-primary'
            : 'text-muted-foreground',
        )}
        onClick={toggleShuffle}
        onContextMenu={(event) => {
          event.preventDefault()
          setOpen(true)
        }}
        onKeyDown={(event) => {
          if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
            event.preventDefault()
            setOpen(true)
          }
        }}
      >
        <Shuffle />
      </button>
      <PopoverContent
        anchor={buttonRef}
        side="top"
        sideOffset={40}
        className="w-40 gap-1 p-1 text-xs"
      >
        <TooltipProvider>
          <div className="flex flex-col gap-1 h-40 overflow-y-auto" role="list" aria-label="Shuffle algorithms">
            {SHUFFLE_MODE_OPTIONS.map((option) => {
              const selected = shuffleMode === option.id
              return (
                <div
                  key={option.id}
                  role="listitem"
                  className={cn(
                    'flex items-center rounded-sm hover:bg-accent',
                    selected && 'bg-accent text-accent-foreground',
                  )}
                >
                  <ShufflePopoverItem
                    isActive={selected}
                    onClick={() => selectMode(option.id)}
                  >
                    {option.label}
                  </ShufflePopoverItem>
                  <Tooltip>
                    <TooltipTrigger
                      render={(
                        <button
                          type="button"
                          aria-label={`About ${option.label}`}
                          className="mr-1.5 flex size-7 shrink-0 items-center justify-center rounded-full text-muted-foreground hover:text-foreground"
                        >
                          <CircleHelp className="size-3" />
                        </button>
                      )}
                    />
                    <TooltipContent side="right" className="max-w-64">
                      {option.description}
                    </TooltipContent>
                  </Tooltip>
                </div>
              )
            })}
          </div>
        </TooltipProvider>
      </PopoverContent>
    </Popover>
  )
}
