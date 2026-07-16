import type { Track } from '@/lib/db'
import { cn } from '@/lib/utils'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { Play } from 'lucide-react'
import { Checkbox } from '@/components/ui/checkbox'
import { TrackThumbnail } from './PlaylistTrackThumbnail'
import { formatTrackDuration } from './track-actions'

export const TRACK_ROW_HEIGHT = 70
/** Space between rows; baked into stride so DnD measuring matches layout. */
export const TRACK_ROW_GAP = 8
export const TRACK_ROW_STRIDE = TRACK_ROW_HEIGHT + TRACK_ROW_GAP
export const TRACK_SELECT_COL = '2.25rem'
export const TRACK_GRID_COLS = `minmax(0, 1.4fr) minmax(0, 1fr) 4.5rem`
export const TRACK_GRID_COLS_SELECT = `${TRACK_SELECT_COL} ${TRACK_GRID_COLS}`

export interface PlaylistTrackRowViewProps {
  track: Track
  isActive: boolean
  isPlaying: boolean
  isSelected: boolean
  selectionMode: boolean
  className?: string
  style?: React.CSSProperties
  onRowClick?: () => void
  onToggleSelected?: (selected: boolean) => void
  onPlayFromThumb?: () => void
}

/** Presentational track row (list item or drag overlay). */
export function PlaylistTrackRowView({
  track,
  isActive,
  isPlaying,
  isSelected,
  selectionMode,
  className,
  style,
  onRowClick,
  onToggleSelected,
  onPlayFromThumb,
}: PlaylistTrackRowViewProps) {
  const showEqualizer = isActive && isPlaying

  return (
    <div
      role="row"
      tabIndex={onRowClick ? 0 : -1}
      onClick={onRowClick}
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
        'group relative grid w-full cursor-default items-center gap-3 rounded-lg px-2.5 transition-colors',
        'border-2 border-transparent hover:bg-card/70',
        isSelected && 'border-primary/50 bg-primary/8',
        isActive && !isSelected && 'bg-accent/40',
        className,
      )}
      style={{
        height: TRACK_ROW_HEIGHT,
        gridTemplateColumns: selectionMode
          ? TRACK_GRID_COLS_SELECT
          : TRACK_GRID_COLS,
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

      <div role="cell" className="flex min-w-0 items-center gap-3">
        <div className="relative shrink-0">
          <TrackThumbnail
            trackId={track.id}
            fileUniqueId={track.file_unique_id}
          />
          <button
            type="button"
            aria-label={showEqualizer ? 'Now playing' : 'Play track'}
            onClick={(event) => {
              event.stopPropagation()
              onPlayFromThumb?.()
            }}
            className={cn(
              'absolute inset-0 flex items-center justify-center rounded-sm bg-background/65 backdrop-blur-[1px] transition-opacity duration-200',
              isActive ? 'opacity-100' : 'opacity-0 group-hover:opacity-100',
            )}
          >
            {showEqualizer
              ? (
                  <span className="equalizer flex h-4 items-end gap-1">
                    <span className="w-[3px] rounded-[1px] bg-foreground" />
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
          </button>
        </div>

        <span
          className={cn(
            'max-w-full truncate text-sm font-medium',
            isActive ? 'text-primary' : 'text-foreground',
          )}
        >
          {track.title ?? 'Unknown Title'}
        </span>
      </div>

      <div role="cell" className="min-w-0">
        <span className="block max-w-full truncate text-sm text-muted-foreground">
          {track.performer ?? 'Unknown Artist'}
        </span>
      </div>

      <div role="cell" className="flex justify-end">
        <span className="w-10 shrink-0 text-right font-mono text-xs tabular-nums text-muted-foreground">
          {formatTrackDuration(track.duration)}
        </span>
      </div>
    </div>
  )
}

export interface PlaylistTrackRowProps {
  track: Track
  isActive: boolean
  isPlaying: boolean
  isSelected: boolean
  selectionMode: boolean
  canReorder: boolean
  virtualStart: number
  className?: string
  onRowClick: () => void
  onToggleSelected: (selected: boolean) => void
  onPlayFromThumb: () => void
}

export function PlaylistTrackRow({
  track,
  isActive,
  isPlaying,
  isSelected,
  selectionMode,
  canReorder,
  virtualStart,
  className,
  onRowClick,
  onToggleSelected,
  onPlayFromThumb,
}: PlaylistTrackRowProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: track.id,
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
        className={cn(isDragging && 'opacity-0')}
        onRowClick={onRowClick}
        onToggleSelected={onToggleSelected}
        onPlayFromThumb={onPlayFromThumb}
      />
    </div>
  )
}
