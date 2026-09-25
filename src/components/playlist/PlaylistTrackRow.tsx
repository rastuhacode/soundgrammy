import type { Track } from '@/lib/db'
import { cn } from '@/lib/utils'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { Ellipsis, GripVertical, Play } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { openContextMenuFromPointerEvent } from './SidebarPlaylistContextMenu'
import { TrackThumbnail } from './PlaylistTrackThumbnail'
import { formatTrackDuration } from './track-actions'

export const TRACK_ROW_HEIGHT = 70
/** Space between rows; baked into stride so DnD measuring matches layout. */
export const TRACK_ROW_GAP = 8
export const TRACK_ROW_STRIDE = TRACK_ROW_HEIGHT + TRACK_ROW_GAP
export const TRACK_GRID_CLASS = 'grid-cols-[minmax(0,1fr)_3rem_2.25rem] md:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_4.5rem_2.25rem]'
export const TRACK_GRID_CLASS_SELECT = 'grid-cols-[2.25rem_minmax(0,1fr)_3rem_2.25rem] md:grid-cols-[2.25rem_minmax(0,1.4fr)_minmax(0,1fr)_4.5rem_2.25rem]'

export interface PlaylistTrackRowViewProps {
  track: Track
  isActive: boolean
  isPlaying: boolean
  isSelected: boolean
  selectionMode: boolean
  touchScreen?: boolean
  canReorder?: boolean
  className?: string
  style?: React.CSSProperties
  onRowClick?: () => void
  onToggleSelected?: (selected: boolean) => void
  onEnterSelection?: () => void
  onTouchDragStart?: React.TouchEventHandler<HTMLButtonElement>
  touchOptions?: React.ReactNode
}

/** Presentational track row. */
export function PlaylistTrackRowView({
  track,
  isActive,
  isPlaying,
  isSelected,
  selectionMode,
  touchScreen = false,
  canReorder = false,
  className,
  style,
  onRowClick,
  onToggleSelected,
  onEnterSelection,
  onTouchDragStart,
  touchOptions,
}: PlaylistTrackRowViewProps) {
  const longPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const touchStart = useRef<{ x: number, y: number } | null>(null)
  const suppressClick = useRef(false)
  const lastTouchAt = useRef<number | null>(null)
  const clearLongPress = () => {
    if (longPressTimer.current !== null) clearTimeout(longPressTimer.current)
    longPressTimer.current = null
    touchStart.current = null
  }

  useEffect(() => () => {
    if (longPressTimer.current !== null) clearTimeout(longPressTimer.current)
  }, [])

  const showEqualizer = isActive && isPlaying
  const trackTitle = track.title ?? 'Unknown Title'
  const trackArtist = track.performer ?? 'Unknown Artist'

  return (
    <div
      role="row"
      tabIndex={onRowClick ? 0 : -1}
      onClickCapture={(event) => {
        if (!event.currentTarget.contains(event.target as Node) || !suppressClick.current) return
        suppressClick.current = false
        event.preventDefault()
        event.stopPropagation()
      }}
      onClick={(event) => {
        // Portaled menu items still bubble through this row in React.
        if (!event.currentTarget.contains(event.target as Node)) return
        onRowClick?.()
      }}
      onPointerDown={(event) => {
        if (!event.currentTarget.contains(event.target as Node)) return
        lastTouchAt.current = event.pointerType === 'touch' ? Date.now() : null
        suppressClick.current = false
        if (event.pointerType !== 'touch' || selectionMode || !onEnterSelection) return
        clearLongPress()
        touchStart.current = { x: event.clientX, y: event.clientY }
        longPressTimer.current = setTimeout(() => {
          longPressTimer.current = null
          touchStart.current = null
          suppressClick.current = true
          onEnterSelection()
        }, 500)
      }}
      onPointerMove={(event) => {
        if (!touchStart.current) return
        if (Math.abs(event.clientX - touchStart.current.x) > 10
          || Math.abs(event.clientY - touchStart.current.y) > 10) {
          clearLongPress()
        }
      }}
      onPointerUp={clearLongPress}
      onPointerCancel={clearLongPress}
      onTouchStart={event => event.stopPropagation()}
      onContextMenuCapture={(event) => {
        if (!touchScreen && (lastTouchAt.current === null || Date.now() - lastTouchAt.current > 1000)) return
        event.preventDefault()
        event.stopPropagation()
      }}
      onKeyDown={(event) => {
        if (!onRowClick) return
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault()
          onRowClick()
        }
      }}
      aria-selected={isSelected}
      aria-label={
        selectionMode
          ? `${isSelected ? 'Deselect' : 'Select'} ${track.title ?? 'track'}`
          : showEqualizer
            ? 'Pause track'
            : 'Play track'
      }
      className={cn(
        'group relative grid w-full cursor-default items-center gap-2 rounded-lg px-2 transition-colors md:gap-3 md:px-2.5',
        selectionMode || (touchScreen && canReorder)
          ? TRACK_GRID_CLASS_SELECT
          : TRACK_GRID_CLASS,
        'border-2 border-transparent hover:bg-card/70',
        isSelected && 'border-primary/50 bg-primary/8',
        isActive && !isSelected && 'bg-accent/40',
        className,
      )}
      style={{
        height: TRACK_ROW_HEIGHT,
        ...style,
      }}
    >
      {selectionMode && (
        <div
          role="cell"
          className="flex size-full items-center justify-center"
          onClick={event => event.stopPropagation()}
        >
          <Checkbox
            checked={isSelected}
            onCheckedChange={(checked) => {
              onToggleSelected?.(checked)
            }}
            aria-label={`Select ${track.title ?? 'track'}`}
            className="animate-in fade-in-0 zoom-in-95 duration-150"
          />
        </div>
      )}

      {!selectionMode && touchScreen && canReorder && (
        <div role="cell" className="flex size-full items-center justify-center">
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            aria-label={`Drag ${track.title ?? 'track'} to reorder`}
            className="size-9 text-muted-foreground touch-none"
            onPointerDown={event => event.stopPropagation()}
            onTouchStart={onTouchDragStart}
            onClick={event => event.stopPropagation()}
          >
            <GripVertical className="size-4" />
          </Button>
        </div>
      )}

      <div role="cell" className="flex min-w-0 items-center gap-3">
        <div className="relative shrink-0">
          <TrackThumbnail
            trackId={track.id}
            fileUniqueId={track.file_unique_id}
          />
          <div
            className={cn(
              'absolute inset-0 flex items-center justify-center rounded-sm bg-background/65 backdrop-blur-[1px] transition-opacity duration-200',
              isActive ? 'opacity-100' : 'opacity-0',
              !selectionMode && 'group-hover:opacity-100',
            )}
          >
            {showEqualizer
              ? (
                  <span className="equalizer flex h-4 items-end gap-1">
                    <span className="w-0.75 rounded-[1px] bg-foreground" />
                    <span />
                    <span />
                  </span>
                )
              : (
                  <Play
                    className={cn(
                      'size-5',
                      isActive
                        ? 'fill-primary text-primary'
                        : 'fill-foreground text-foreground',
                    )}
                  />
                )}
          </div>
        </div>

        <span className="flex min-w-0 flex-col">
          <span
            className={cn(
              'max-w-full truncate text-sm font-medium',
              isActive ? 'text-primary' : 'text-foreground',
            )}
            title={trackTitle}
          >
            {trackTitle}
          </span>
          <span className="truncate text-xs text-muted-foreground md:hidden" title={trackArtist}>
            {trackArtist}
          </span>
        </span>
      </div>

      <div role="cell" className="hidden min-w-0 md:block">
        <span className="block max-w-full truncate text-sm text-muted-foreground" title={trackArtist}>
          {trackArtist}
        </span>
      </div>

      <div role="cell" className="flex justify-end">
        <span className="w-10 shrink-0 text-right font-mono text-xs tabular-nums text-muted-foreground">
          {formatTrackDuration(track.duration)}
        </span>
      </div>

      <div role="cell" className="flex justify-center">
        {!selectionMode && touchScreen && touchOptions}
        {!selectionMode && !touchScreen && (
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            aria-label={`${track.title ?? 'Track'} options`}
            aria-haspopup="menu"
            className={cn(
              'text-muted-foreground opacity-0 transition-opacity',
              !selectionMode && 'touch-visible-option size-9 opacity-100 md:size-6 md:opacity-0 md:group-hover:opacity-100 md:focus-visible:opacity-100',
            )}
            onClick={(event) => {
              openContextMenuFromPointerEvent(event, event.currentTarget)
            }}
            onPointerDown={(event) => {
              lastTouchAt.current = event.pointerType === 'touch' ? Date.now() : null
              event.stopPropagation()
            }}
          >
            <Ellipsis className="size-4" />
          </Button>
        )}
      </div>
    </div>
  )
}

export interface PlaylistTrackRowProps {
  track: Track
  sortableId: string | number
  isActive: boolean
  isPlaying: boolean
  isSelected: boolean
  selectionMode: boolean
  touchScreen: boolean
  canReorder: boolean
  virtualStart: number
  className?: string
  onRowClick: () => void
  onToggleSelected: (selected: boolean) => void
  onEnterSelection: () => void
  touchOptions?: React.ReactNode
}

export function PlaylistTrackRow({
  track,
  sortableId,
  isActive,
  isPlaying,
  isSelected,
  selectionMode,
  touchScreen,
  canReorder,
  virtualStart,
  className,
  onRowClick,
  onToggleSelected,
  onEnterSelection,
  touchOptions,
}: PlaylistTrackRowProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: sortableId,
    disabled: !canReorder,
  })

  return (
    <div
      ref={setNodeRef}
      className={cn(
        'absolute left-0 top-0 w-full',
        canReorder && 'cursor-grab active:cursor-grabbing',
        isDragging && 'z-10',
        className,
      )}
      style={{
        // Stride height (row + gap) so virtualizer and sortable strategy agree.
        height: TRACK_ROW_STRIDE,
        top: virtualStart,
        transform: CSS.Transform.toString(transform),
        transition,
      }}
      {...(canReorder ? { ...attributes, ...listeners } : {})}
    >
      <PlaylistTrackRowView
        track={track}
        isActive={isActive}
        isPlaying={isPlaying}
        isSelected={isSelected}
        selectionMode={selectionMode}
        touchScreen={touchScreen}
        canReorder={canReorder}
        className={cn(
          isDragging && 'cursor-grabbing bg-muted opacity-95 shadow-md',
        )}
        onRowClick={onRowClick}
        onToggleSelected={onToggleSelected}
        onEnterSelection={onEnterSelection}
        touchOptions={touchOptions}
        onTouchDragStart={canReorder && listeners?.onTouchStart
          ? event => listeners.onTouchStart(event)
          : undefined}
      />
    </div>
  )
}
