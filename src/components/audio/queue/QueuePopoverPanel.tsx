import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core'
import {
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { useLocalStorage } from '@mantine/hooks'
import { useVirtualizer } from '@tanstack/react-virtual'
import { ChevronRight, ListPlus, ListX, MoreHorizontal, Trash2, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { z } from 'zod'
import type { Track } from '@/lib/db'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import {
  Collapsible,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { TrackThumbnail } from '@/components/playlist/PlaylistTrackThumbnail'
import { usePlayerStore } from '@/stores/player-store'
import {
  buildQueueListItems,
  estimateQueueListItemSize,
  type QueueListItem,
  type QueueSectionId,
} from './queue-list-items'
import { SaveQueueAsPlaylistDialog } from './SaveQueueAsPlaylistDialog'

const QUEUE_SECTIONS_STORAGE_KEY = 'soundgrammy-queue-sections'

const queueSectionPrefsSchema = z.object({
  historyOpen: z.boolean(),
  upNextOpen: z.boolean(),
})

type QueueSectionPrefs = z.infer<typeof queueSectionPrefsSchema>

const QUEUE_SECTION_PREFS_DEFAULT: QueueSectionPrefs = {
  historyOpen: true,
  upNextOpen: true,
}

function parseQueueSectionPrefs(stored: string | undefined): QueueSectionPrefs {
  if (stored === undefined) return QUEUE_SECTION_PREFS_DEFAULT
  try {
    const parsed = queueSectionPrefsSchema.safeParse(JSON.parse(stored))
    return parsed.success ? parsed.data : QUEUE_SECTION_PREFS_DEFAULT
  }
  catch {
    return QUEUE_SECTION_PREFS_DEFAULT
  }
}

export interface QueuePopoverPanelProps {
  onClose: () => void
  hasTracks: boolean
}

export function QueuePopoverPanel({ onClose, hasTracks }: QueuePopoverPanelProps) {
  const queue = usePlayerStore(state => state.queue)
  const jumpToQueueIndex = usePlayerStore(state => state.jumpToQueueIndex)
  const removeFromQueue = usePlayerStore(state => state.removeFromQueue)
  const reorderQueue = usePlayerStore(state => state.reorderQueue)
  const clearUpNext = usePlayerStore(state => state.clearUpNext)
  const clearQueue = usePlayerStore(state => state.clearQueue)
  const [saveOpen, setSaveOpen] = useState(false)
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null)
  const [sectionPrefs, setSectionPrefs] = useLocalStorage<QueueSectionPrefs>({
    key: QUEUE_SECTIONS_STORAGE_KEY,
    defaultValue: QUEUE_SECTION_PREFS_DEFAULT,
    getInitialValueInEffect: false,
    deserialize: parseQueueSectionPrefs,
  })
  const { historyOpen, upNextOpen } = sectionPrefs
  const scrollRef = useRef<HTMLDivElement>(null)
  const didScrollToCurrentRef = useRef(false)

  const setHistoryOpen = (open: boolean) => {
    setSectionPrefs(prev => ({ ...prev, historyOpen: open }))
  }
  const setUpNextOpen = (open: boolean) => {
    setSectionPrefs(prev => ({ ...prev, upNextOpen: open }))
  }

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  )

  const listItems = useMemo(
    () => buildQueueListItems({
      trackCount: queue.tracks.length,
      cursor: queue.cursor,
      historyOpen,
      upNextOpen,
    }),
    [queue.tracks.length, queue.cursor, historyOpen, upNextOpen],
  )

  const virtualizer = useVirtualizer({
    count: listItems.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: index => estimateQueueListItemSize(listItems[index]),
    getItemKey: (index) => {
      const item = listItems[index]
      if (!item) return index
      return item.type === 'header'
        ? `header-${item.section}`
        : `track-${item.queueIndex}`
    },
    overscan: 8,
  })

  useEffect(() => {
    if (didScrollToCurrentRef.current || listItems.length === 0) return
    const currentListIndex = listItems.findIndex(
      item => item.type === 'track' && item.queueIndex === queue.cursor,
    )
    if (currentListIndex < 0) return
    didScrollToCurrentRef.current = true
    requestAnimationFrame(() => {
      virtualizer.scrollToIndex(currentListIndex, { align: 'center' })
    })
  }, [listItems, queue.cursor, virtualizer])

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event
    if (!over || active.id === over.id) return
    reorderQueue(Number(active.id), Number(over.id))
  }

  if (!hasTracks) {
    return (
      <div className="flex flex-col gap-2 p-4">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-medium">Queue</h2>
          <Button variant="ghost" size="icon-sm" aria-label="Close queue" onClick={onClose}>
            <X className="size-4" />
          </Button>
        </div>
        <p className="text-sm text-muted-foreground">
          Nothing in the queue. Play a playlist or add tracks to get started.
        </p>
      </div>
    )
  }

  const positionLabel = `${queue.cursor + 1} of ${queue.tracks.length}`
  const sourceLabel = queue.source?.name ?? null
  const sortableIds = queue.tracks.map((_, index) => String(index))

  return (
    <div
      className="flex max-h-[min(28rem,70vh)] flex-col"
      onPointerLeave={() => setHoveredIndex(null)}
    >
      <div className="flex items-start justify-between gap-2 border-b border-border px-3 py-2.5">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h2 className="text-sm font-medium">Queue</h2>
            <span className="font-mono text-[11px] text-muted-foreground tabular-nums">
              {positionLabel}
            </span>
          </div>
          {sourceLabel && (
            <p className="truncate text-xs text-muted-foreground">
              From
              {' '}
              {sourceLabel}
            </p>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-0.5">
          <DropdownMenu>
            <DropdownMenuTrigger
              render={(
                <Button variant="ghost" size="icon-sm" aria-label="Queue actions">
                  <MoreHorizontal className="size-4" />
                </Button>
              )}
            />
            <DropdownMenuContent align="end" className="w-48">
              <DropdownMenuItem onClick={() => setSaveOpen(true)}>
                <ListPlus className="size-4" />
                Save as playlist
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                disabled={queue.cursor >= queue.tracks.length - 1}
                onClick={clearUpNext}
              >
                <ListX className="size-4" />
                Clear up next
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={() => {
                  clearQueue()
                  onClose()
                }}
              >
                <Trash2 className="size-4" />
                Clear
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button variant="ghost" size="icon-sm" aria-label="Close queue" onClick={onClose}>
            <X className="size-4" />
          </Button>
        </div>
      </div>

      <div
        ref={scrollRef}
        className="min-h-0 grow overflow-y-auto px-1 py-1"
        onScroll={() => setHoveredIndex(null)}
      >
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          onDragEnd={handleDragEnd}
        >
          <SortableContext items={sortableIds} strategy={verticalListSortingStrategy}>
            <div
              className="relative w-full"
              style={{ height: `${virtualizer.getTotalSize()}px` }}
            >
              {virtualizer.getVirtualItems().map((virtualRow) => {
                const item = listItems[virtualRow.index]
                if (!item) return null

                return (
                  <div
                    key={virtualRow.key}
                    data-index={virtualRow.index}
                    ref={virtualizer.measureElement}
                    className="absolute top-0 left-0 w-full"
                    style={{
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                  >
                    <QueueListRow
                      item={item}
                      track={item.type === 'track'
                        ? queue.tracks[item.queueIndex] ?? null
                        : null}
                      cursor={queue.cursor}
                      historyOpen={historyOpen}
                      upNextOpen={upNextOpen}
                      onHistoryOpenChange={setHistoryOpen}
                      onUpNextOpenChange={setUpNextOpen}
                      hoveredIndex={hoveredIndex}
                      onHoverIndex={setHoveredIndex}
                      onPlay={jumpToQueueIndex}
                      onRemove={index => removeFromQueue([index])}
                    />
                  </div>
                )
              })}
            </div>
          </SortableContext>
        </DndContext>
      </div>

      <SaveQueueAsPlaylistDialog open={saveOpen} onOpenChange={setSaveOpen} />
    </div>
  )
}

function QueueListRow({
  item,
  track,
  cursor,
  historyOpen,
  upNextOpen,
  onHistoryOpenChange,
  onUpNextOpenChange,
  hoveredIndex,
  onHoverIndex,
  onPlay,
  onRemove,
}: {
  item: QueueListItem
  track: Track | null
  cursor: number
  historyOpen: boolean
  upNextOpen: boolean
  onHistoryOpenChange: (open: boolean) => void
  onUpNextOpenChange: (open: boolean) => void
  hoveredIndex: number | null
  onHoverIndex: (index: number | null | ((prev: number | null) => number | null)) => void
  onPlay: (index: number) => void
  onRemove: (index: number) => void
}) {
  if (item.type === 'header') {
    return (
      <QueueSectionHeader
        section={item.section}
        count={item.count}
        open={item.section === 'history'
          ? historyOpen
          : item.section === 'upNext'
            ? upNextOpen
            : true}
        onOpenChange={item.section === 'history'
          ? onHistoryOpenChange
          : item.section === 'upNext'
            ? onUpNextOpenChange
            : undefined}
      />
    )
  }

  if (!track) return null

  return (
    <QueueEntryRow
      track={track}
      index={item.queueIndex}
      isCurrent={item.queueIndex === cursor}
      showRemove={hoveredIndex === item.queueIndex}
      onHoverChange={(hovered) => {
        if (hovered) onHoverIndex(item.queueIndex)
        else onHoverIndex(prev => (prev === item.queueIndex ? null : prev))
      }}
      onPlay={() => onPlay(item.queueIndex)}
      onRemove={() => onRemove(item.queueIndex)}
    />
  )
}

function QueueSectionHeader({
  section,
  count,
  open,
  onOpenChange,
}: {
  section: QueueSectionId
  count: number
  open: boolean
  onOpenChange?: (open: boolean) => void
}) {
  const label = section === 'history'
    ? 'History'
    : section === 'now'
      ? 'Now playing'
      : 'Up next'

  if (!onOpenChange) {
    return (
      <p className="flex h-7 items-center px-2 text-[10px] font-medium tracking-wide text-muted-foreground uppercase">
        {label}
      </p>
    )
  }

  return (
    <Collapsible open={open} onOpenChange={onOpenChange}>
      <CollapsibleTrigger
        className={cn(
          'flex h-7 w-full items-center gap-1 rounded-md px-2',
          'text-[10px] font-medium tracking-wide text-muted-foreground uppercase',
          'hover:bg-muted/60 hover:text-foreground',
        )}
      >
        <ChevronRight
          className={cn(
            'size-3 shrink-0 transition-transform',
            open && 'rotate-90',
          )}
        />
        <span className="min-w-0 grow text-left">{label}</span>
        <span className="font-mono text-[10px] tabular-nums normal-case">
          {count}
        </span>
      </CollapsibleTrigger>
    </Collapsible>
  )
}

function QueueEntryRow({
  track,
  index,
  isCurrent,
  showRemove,
  onHoverChange,
  onPlay,
  onRemove,
}: {
  track: Track
  index: number
  isCurrent: boolean
  showRemove: boolean
  onHoverChange: (hovered: boolean) => void
  onPlay: () => void
  onRemove: () => void
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: String(index) })

  return (
    <div
      ref={setNodeRef}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
        height: estimateQueueListItemSize({ type: 'track', queueIndex: index }),
      }}
      className={cn(
        'flex items-center gap-2 rounded-md px-2',
        isCurrent && 'bg-primary/10',
        isDragging && 'z-10 opacity-80',
      )}
      onPointerEnter={() => onHoverChange(true)}
      onPointerLeave={() => onHoverChange(false)}
    >
      <button
        type="button"
        className="flex min-w-0 grow items-center gap-2 text-left active:cursor-grabbing"
        onClick={onPlay}
        {...attributes}
        {...listeners}
      >
        <TrackThumbnail trackId={track.id} fileUniqueId={track.file_unique_id} />
        <div className="min-w-0 grow">
          <p className={cn('truncate text-sm', isCurrent && 'font-medium text-primary')}>
            {track.title ?? 'Unknown title'}
          </p>
          <p className="truncate text-xs text-muted-foreground">
            {track.performer ?? 'Unknown artist'}
          </p>
        </div>
      </button>
      {showRemove && !isDragging
        ? (
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Remove from queue"
              className="shrink-0"
              onClick={(event) => {
                event.stopPropagation()
                onRemove()
              }}
            >
              <X className="size-3.5" />
            </Button>
          )
        : (
            <span aria-hidden className="size-7 shrink-0" />
          )}
    </div>
  )
}
