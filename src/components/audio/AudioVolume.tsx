import { useEffect, useRef, useState } from 'react'
import { Volume1, Volume2, VolumeX } from 'lucide-react'
import type { PopoverRoot } from '@base-ui/react/popover'
import { cn } from '@/lib/utils'
import {
  consumeVolumeWheelDelta,
  normalizeVolume,
  VOLUME_MAX,
  VOLUME_MIN,
} from '@/lib/volume'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
  type PopoverClasses,
} from '@/components/ui/popover'

export interface AudioVolumeProps {
  volume: number
  onVolumeChange: (volume: number) => void
  onMuteToggle: () => void
  classes?: PopoverClasses
}

export function AudioVolume(props: AudioVolumeProps) {
  const [open, setOpen] = useState(false)
  const [editingExact, setEditingExact] = useState(false)
  const [exactDraft, setExactDraft] = useState(String(props.volume))
  const exactInputRef = useRef<HTMLInputElement>(null)
  const valueChipRef = useRef<HTMLButtonElement>(null)
  const volumeRef = useRef(normalizeVolume(props.volume))
  const wheelRemainderRef = useRef(0)
  const cancelBlurRef = useRef(false)
  const isMuted = props.volume === 0
  const VolumeIcon = isMuted ? VolumeX : props.volume < 50 ? Volume1 : Volume2

  useEffect(() => {
    volumeRef.current = normalizeVolume(props.volume)
  }, [props.volume])

  useEffect(() => {
    if (!editingExact) return
    exactInputRef.current?.focus()
    exactInputRef.current?.select()
  }, [editingExact])

  function changeVolume(value: number) {
    const nextVolume = normalizeVolume(value)
    volumeRef.current = nextVolume
    if (editingExact) setExactDraft(String(nextVolume))
    if (nextVolume !== normalizeVolume(props.volume)) {
      props.onVolumeChange(nextVolume)
    }
  }

  function finishExactEdit(restoreFocus: boolean) {
    const trimmed = exactDraft.trim()
    const parsed = Number(trimmed)
    if (trimmed !== '' && Number.isFinite(parsed)) changeVolume(parsed)
    else setExactDraft(String(volumeRef.current))

    setEditingExact(false)
    if (restoreFocus) {
      requestAnimationFrame(() => valueChipRef.current?.focus())
    }
  }

  function cancelExactEdit() {
    cancelBlurRef.current = true
    setExactDraft(String(volumeRef.current))
    setEditingExact(false)
    queueMicrotask(() => {
      cancelBlurRef.current = false
    })
    requestAnimationFrame(() => valueChipRef.current?.focus())
  }

  function handleOpenChange(
    nextOpen: boolean,
    details: PopoverRoot.ChangeEventDetails,
  ) {
    if (details.reason === 'trigger-press') return
    if (!nextOpen && editingExact) {
      if (details.reason === 'trigger-hover') return
      if (details.reason === 'escape-key') cancelExactEdit()
      else finishExactEdit(false)
    }
    setOpen(nextOpen)
  }

  function handleWheel(e: React.WheelEvent<HTMLElement>) {
    if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return

    e.preventDefault()
    const result = consumeVolumeWheelDelta(
      e.deltaY,
      e.deltaMode,
      wheelRemainderRef.current,
    )
    wheelRemainderRef.current = result.remainder
    if (result.step !== 0) changeVolume(volumeRef.current + result.step)
  }

  function resetWheelRemainder() {
    wheelRemainderRef.current = 0
  }

  function handleExactKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === 'Enter') {
      e.preventDefault()
      finishExactEdit(true)
    }
    else if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      cancelExactEdit()
    }
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
            onWheel={handleWheel}
            onPointerLeave={resetWheelRemainder}
          >
            <VolumeIcon className="size-5 shrink-0" />
          </Button>
        )}
      />
      <PopoverContent
        side="top"
        align="center"
        className="w-10 items-center gap-1.5 py-2"
        classes={props.classes}
        onWheel={handleWheel}
        onPointerLeave={resetWheelRemainder}
      >
        {editingExact
          ? (
              <div className="relative w-10">
                <Input
                  ref={exactInputRef}
                  type="number"
                  min={VOLUME_MIN}
                  max={VOLUME_MAX}
                  step={1}
                  value={exactDraft}
                  onChange={e => setExactDraft(e.target.value)}
                  onKeyDown={handleExactKeyDown}
                  onBlur={() => {
                    if (cancelBlurRef.current) {
                      cancelBlurRef.current = false
                      return
                    }
                    finishExactEdit(false)
                  }}
                  aria-label="Exact volume percentage"
                  className="h-7 pr-5 pl-1.5 text-center font-mono text-xs tabular-nums [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
                />
                <span
                  aria-hidden="true"
                  className="pointer-events-none absolute inset-y-0 right-1.5 flex items-center text-xs text-muted-foreground"
                >
                  %
                </span>
              </div>
            )
          : (
              <Button
                ref={valueChipRef}
                variant="ghost"
                size="xs"
                className="h-7 px-2 font-mono text-xs tabular-nums text-muted-foreground hover:text-foreground"
                aria-label={`Set exact volume, currently ${props.volume}%`}
                title="Set exact volume"
                onClick={() => {
                  cancelBlurRef.current = false
                  setExactDraft(String(volumeRef.current))
                  setEditingExact(true)
                  setOpen(true)
                }}
              >
                {`${props.volume}%`}
              </Button>
            )}
        <div className="flex h-24 w-8 items-center justify-center">
          <input
            type="range"
            className={
              cn(
                'appearance-none outline-none w-24 h-0.5 -rotate-90',
                '[background:linear-gradient(to_right,var(--primary)_0%,var(--primary)_var(--progress,0%),color-mix(in_oklch,var(--foreground)_18%,transparent)_var(--progress,0%),color-mix(in_oklch,var(--foreground)_18%,transparent)_100%)]',
                '[&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-primary [&::-webkit-slider-thumb]:appearance-none',
                '[&::-webkit-slider-thumb]:transition-transform [&::-webkit-slider-thumb]:duration-150 [&::-webkit-slider-thumb:hover]:scale-125',
                '[&::-moz-range-thumb]:size-3 [&::-webkit-slider-thumb]:size-3',
                '[&::-moz-range-thumb]:border-none [&::-moz-range-thumb]:bg-primary [&::-moz-range-thumb]:rounded-full',
              )
            }
            min={0}
            max={100}
            step={1}
            value={props.volume}
            onChange={e => changeVolume(Number(e.target.value))}
            autoComplete="off"
            aria-label="Volume"
            style={{ '--progress': `${props.volume}%` } as React.CSSProperties}
          />
        </div>
      </PopoverContent>
    </Popover>
  )
}
