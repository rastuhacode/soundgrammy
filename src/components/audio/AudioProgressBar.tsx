import { useEffect, useMemo, useRef, useState } from 'react'
import { cn } from '@/lib/utils'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'

export interface AudioBufferedRange {
  start: number
  end: number
}

export interface AudioProgressBarProps {
  currentTime: number
  duration: number
  bufferedRanges?: AudioBufferedRange[]
  showInitialLoading?: boolean
  onSeek: (time: number) => void
  onSeekStart?: () => void
  onSeekEnd?: () => void
  className?: string
}

// Virtual anchor tooltip positioning offset. Set to remove tooltip flickering
const HOVER_ZONE_EXTENSION = 3 // 3px

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds)) return '0:00'
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

interface PointerAnchor {
  clientX: number
  barRect: DOMRect
}

function createVirtualAnchor({ clientX, barRect }: PointerAnchor) {
  const y = barRect.top + barRect.height / 3
  return {
    getBoundingClientRect: () => DOMRect.fromRect({
      x: clientX,
      y,
      width: 0,
      height: 0,
    }),
  }
}

function isInsideHoverZone(
  clientX: number,
  clientY: number,
  rect: DOMRect,
) {
  return clientX >= rect.left
    && clientX <= rect.right
    && clientY >= rect.top - HOVER_ZONE_EXTENSION
    && clientY <= rect.bottom
}

function updateHoverFromPointer(
  e: { clientX: number },
  rect: DOMRect,
  duration: number,
) {
  const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width))
  return {
    hoverTime: ratio * duration,
    pointerAnchor: { clientX: e.clientX, barRect: rect } satisfies PointerAnchor,
  }
}

export function AudioProgressBar({
  currentTime,
  duration,
  bufferedRanges = [],
  showInitialLoading = false,
  onSeek,
  onSeekStart,
  onSeekEnd,
  className,
}: AudioProgressBarProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [isDragging, setIsDragging] = useState(false)
  const [isHovering, setIsHovering] = useState(false)
  const [hoverTime, setHoverTime] = useState(0)
  const [pointerAnchor, setPointerAnchor] = useState<PointerAnchor | null>(null)

  const tooltipOpen = pointerAnchor !== null && isHovering && !isDragging && duration > 0
  const virtualAnchor = useMemo(
    () => (pointerAnchor ? createVirtualAnchor(pointerAnchor) : undefined),
    [pointerAnchor],
  )

  const progress
    = duration > 0 ? Math.min(100, (currentTime / duration) * 100) : 0

  const handleSeek = (e: React.FormEvent<HTMLInputElement>) => {
    onSeek(Number(e.currentTarget.value))
  }

  const hideTooltip = () => {
    setIsHovering(false)
    setPointerAnchor(null)
  }

  const handleSeekStart = () => {
    hideTooltip()
    setIsDragging(true)
    onSeekStart?.()
  }

  const finishSeek = () => {
    setIsDragging(false)
    onSeekEnd?.()
  }

  const handlePointerSeekEnd = (e: React.PointerEvent<HTMLInputElement>) => {
    setIsDragging(false)
    const rect = containerRef.current?.getBoundingClientRect()
    if (rect && duration > 0 && isInsideHoverZone(e.clientX, e.clientY, rect)) {
      const next = updateHoverFromPointer(e, rect, duration)
      setHoverTime(next.hoverTime)
      setPointerAnchor(next.pointerAnchor)
      setIsHovering(true)
    }
    onSeekEnd?.()
  }

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect()
    if (rect.width <= 0 || duration <= 0 || isDragging) return
    const next = updateHoverFromPointer(e, rect, duration)
    setHoverTime(next.hoverTime)
    setPointerAnchor(next.pointerAnchor)
    setIsHovering(true)
  }

  useEffect(() => {
    if (!isHovering) return

    const handleDocumentPointerMove = (e: PointerEvent) => {
      const rect = containerRef.current?.getBoundingClientRect()
      if (!rect) return

      if (!isInsideHoverZone(e.clientX, e.clientY, rect)) {
        hideTooltip()
        return
      }

      if (!isDragging && duration > 0) {
        const next = updateHoverFromPointer(e, rect, duration)
        setHoverTime(next.hoverTime)
        setPointerAnchor(next.pointerAnchor)
      }
    }

    document.addEventListener('pointermove', handleDocumentPointerMove)
    return () => document.removeEventListener('pointermove', handleDocumentPointerMove)
  }, [duration, isDragging, isHovering])

  return (
    <TooltipProvider>
      <Tooltip
        open={tooltipOpen}
        onOpenChange={() => {
          // Open/close is controlled via pointer zone tracking above.
        }}
        disabled={duration <= 0}
      >
        <TooltipTrigger
          render={(triggerProps) => {
            const {
              onPointerDown,
              ref,
              ...rest
            } = triggerProps

            return (
              <div
                {...rest}
                ref={(node) => {
                  containerRef.current = node
                  if (typeof ref === 'function') ref(node)
                  else if (ref) ref.current = node
                }}
                className={cn(
                  'group/audiobar absolute h-8 -top-4 w-full z-100',
                  className,
                )}
                onPointerEnter={handlePointerMove}
                onPointerMove={handlePointerMove}
                onPointerDown={(e) => {
                  onPointerDown?.(e)
                  hideTooltip()
                }}
              >
                <div
                  aria-hidden
                  className="pointer-events-none absolute inset-x-0 top-4 h-1 overflow-hidden bg-foreground/15"
                >
                  {showInitialLoading && (
                    <div className="barbershop-buffer absolute inset-0 opacity-35" />
                  )}
                  {bufferedRanges.map((range, index) => {
                    const start = duration > 0
                      ? Math.max(0, Math.min(100, (range.start / duration) * 100))
                      : 0
                    const end = duration > 0
                      ? Math.max(start, Math.min(100, (range.end / duration) * 100))
                      : 0
                    return (
                      <div
                        key={`${index}:${range.start}:${range.end}`}
                        className="absolute inset-y-0 bg-foreground/30"
                        style={{ left: `${start}%`, width: `${end - start}%` }}
                      />
                    )
                  })}
                  <div
                    className="absolute inset-y-0 left-0 bg-primary transition-all duration-200"
                    style={{ width: `${progress}%` }}
                  />
                </div>

                <div
                  aria-hidden
                  className={cn(
                    'pointer-events-none absolute top-[18px] z-10 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-primary opacity-0 transition-all duration-200 group-hover/audiobar:opacity-100',
                    isDragging && 'scale-125 opacity-100',
                  )}
                  style={{ left: `${progress}%` }}
                />

                <input
                  type="range"
                  className="absolute inset-0 h-full w-full appearance-none opacity-0 [&::-moz-range-thumb]:h-4 [&::-moz-range-thumb]:w-0 [&::-moz-range-thumb]:border-0 [&::-moz-range-thumb]:bg-transparent [&::-moz-range-track]:bg-transparent [&::-webkit-slider-runnable-track]:bg-transparent [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-0 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:bg-transparent"
                  min={0}
                  max={duration || 0}
                  step={0.1}
                  value={currentTime}
                  onInput={handleSeek}
                  onPointerDown={handleSeekStart}
                  onPointerUp={handlePointerSeekEnd}
                  onPointerCancel={handlePointerSeekEnd}
                  onTouchEnd={finishSeek}
                  onKeyDown={handleSeekStart}
                  onKeyUp={finishSeek}
                  onBlur={finishSeek}
                  aria-label="Seek"
                  aria-valuemin={0}
                  aria-valuemax={duration || 0}
                  aria-valuenow={currentTime}
                />
              </div>
            )
          }}
        />
        <TooltipContent
          side="top"
          anchor={virtualAnchor}
          className="font-mono tabular-nums p-0 bg-transparent text-foreground **:data-[slot=tooltip-arrow]:bg-transparent data-closed:animate-none data-closed:opacity-0 data-closed:duration-0"
        >
          {formatTime(hoverTime)}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}
