import type { TrackMenuHandlers } from './TrackMenuContent'
import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  type Modifier,
  KeyboardSensor,
  MouseSensor,
  TouchSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core'
import {
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable'
import {
  columnSizingFeature,
  flexRender,
  rowSelectionFeature,
  rowSortingFeature,
  tableFeatures,
  useTable,
  type ColumnDef,
  type OnChangeFn,
  type RowSelectionState,
  type SortingState,
} from '@tanstack/react-table'
import { useVirtualizer } from '@tanstack/react-virtual'
import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-react'
import { useEffect, useMemo, useRef } from 'react'
import { useTouchScreen } from '@/hooks/use-touch-screen'
import { useCompactDisplay } from '@/hooks/use-compact-display'
import type { Track } from '@/lib/db'
import type { PlaylistEntry } from '@/lib/playlists'
import type { ResolvedSelectedPlaylist } from '@/stores/playlists-store'
import { cn } from '@/lib/utils'
import { Checkbox } from '@/components/ui/checkbox'
import { ScrollArea } from '@/components/ui/scroll-area'
import { PlaylistTrackContextMenu } from './PlaylistTrackContextMenu'
import { PlaylistTrackDropdownMenu } from './PlaylistTrackDropdownMenu'
import {
  TRACK_GRID_CLASS,
  TRACK_GRID_CLASS_SELECT,
  TRACK_ROW_STRIDE,
  COMPACT_TRACK_ROW_STRIDE,
  PlaylistTrackRow,
} from './PlaylistTrackRow'
import {
  getTrackSortableIds,
  reorderByIndex,
  type CustomPlaylistRef,
} from '@/lib/playlist-track-actions'

const playlistTableFeatures = tableFeatures({
  columnSizingFeature,
  rowSelectionFeature,
  rowSortingFeature,
})

const restrictToVerticalAxis: Modifier = ({ transform }) => ({
  ...transform,
  x: 0,
})

export interface PlaylistTracksTableProps {
  hideHeader?: boolean
  entries: PlaylistEntry[]
  currentPlaylist: ResolvedSelectedPlaylist
  customPlaylists: CustomPlaylistRef[]
  /** Membership index of the now-playing row; null when nothing should highlight. */
  playingSourceIndex: number | null
  isPlaying: boolean
  isTrackLiked: (trackId: number) => boolean
  selectionMode: boolean
  rowSelection: RowSelectionState
  onRowSelectionChange: OnChangeFn<RowSelectionState>
  onTrackSelect: (sourceIndex: number, selected: boolean, extend: boolean) => void
  sorting: SortingState
  onSortingChange: OnChangeFn<SortingState>
  canReorder: boolean
  onReorderTracks: (
    trackIds: number[],
    move: { fromIndex: number, toIndex: number },
  ) => void
  onEnterSelection: (sourceIndex: number) => void
  onTrackPlay: (track: Track, startIndex: number) => void
  trackActions: TrackMenuHandlers
}

function SortIcon({ sorted }: { sorted: false | 'asc' | 'desc' }) {
  if (sorted === 'asc') return <ArrowUp className="size-3.5 opacity-80" />
  if (sorted === 'desc') return <ArrowDown className="size-3.5 opacity-80" />
  return <ArrowUpDown className="size-3.5 opacity-40" />
}

export function PlaylistTracksTable({
  hideHeader = false,
  entries,
  currentPlaylist,
  customPlaylists,
  playingSourceIndex,
  isPlaying,
  isTrackLiked,
  selectionMode,
  rowSelection,
  onRowSelectionChange,
  onTrackSelect,
  sorting,
  onSortingChange,
  canReorder,
  onReorderTracks,
  onEnterSelection,
  onTrackPlay,
  trackActions,
}: PlaylistTracksTableProps) {
  const tracks = useMemo(() => entries.map(entry => entry.track), [entries])
  const scrollRef = useRef<HTMLDivElement>(null)
  const touchScreen = useTouchScreen()
  const { isCompact } = useCompactDisplay()
  const rowStride = isCompact ? COMPACT_TRACK_ROW_STRIDE : TRACK_ROW_STRIDE

  const columns = useMemo<ColumnDef<typeof playlistTableFeatures, Track>[]>(() => {
    const defs: ColumnDef<typeof playlistTableFeatures, Track>[] = []

    if (selectionMode) {
      defs.push({
        id: 'select',
        header: ({ table }) => (
          <Checkbox
            checked={table.getIsAllRowsSelected()}
            indeterminate={table.getIsSomeRowsSelected() && !table.getIsAllRowsSelected()}
            onCheckedChange={(checked) => {
              table.toggleAllRowsSelected(checked)
            }}
            aria-label="Select all tracks"
          />
        ),
        cell: () => null,
        enableSorting: false,
        size: 36,
      })
    }
    else if (touchScreen && canReorder) {
      defs.push({
        id: 'drag',
        header: () => <span className="sr-only">Reorder tracks</span>,
        cell: () => null,
        enableSorting: false,
        size: 36,
      })
    }

    defs.push(
      {
        accessorKey: 'title',
        header: 'Title',
        cell: () => null,
      },
      {
        accessorKey: 'performer',
        header: 'Artist',
        cell: () => null,
      },
      {
        accessorKey: 'duration',
        header: 'Time',
        cell: () => null,
      },
      {
        id: 'actions',
        header: () => <span className="sr-only">Track options</span>,
        cell: () => null,
        enableSorting: false,
      },
    )

    return defs
  }, [selectionMode, touchScreen, canReorder])

  const table = useTable({
    features: playlistTableFeatures,
    data: tracks,
    columns,
    state: {
      sorting,
      rowSelection,
    },
    getRowId: (_row, index) => String(entries[index]!.sourceIndex),
    manualSorting: true,
    enableRowSelection: selectionMode,
    onSortingChange,
    onRowSelectionChange,
  })

  const rows = table.getRowModel().rows
  const rowSortableIds = useMemo(
    () => getTrackSortableIds(tracks.map(track => track.id)),
    [tracks],
  )
  const sortableIds = rows.map(row => rowSortableIds[row.index]!)

  const sensors = useSensors(
    useSensor(MouseSensor, {
      activationConstraint: { distance: 6 },
    }),
    useSensor(TouchSensor, {
      activationConstraint: { delay: 250, tolerance: 5 },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  )

  // TanStack Virtual intentionally returns live functions
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    // Stride includes former gap so item height matches sortable strategy shifts.
    estimateSize: () => rowStride,
    overscan: 8,
  })

  useEffect(() => {
    virtualizer.measure()
  }, [rowStride, virtualizer])

  const headerGridClass = selectionMode || (touchScreen && canReorder)
    ? TRACK_GRID_CLASS_SELECT
    : TRACK_GRID_CLASS

  const handleDragEnd = (event: DragEndEvent) => {
    if (!canReorder) return
    const { active, over } = event
    if (!over || active.id === over.id) return

    const fromIndex = sortableIds.indexOf(String(active.id))
    const toIndex = sortableIds.indexOf(String(over.id))
    if (fromIndex < 0 || toIndex < 0) return

    const trackIds = tracks.map(track => track.id)
    const next = reorderByIndex(trackIds, fromIndex, toIndex)
    if (
      fromIndex === toIndex
      || next.length !== trackIds.length
    ) {
      return
    }
    // Always notify — duplicate ids can leave the id list unchanged while the
    // playing membership index must still move with the drag.
    onReorderTracks(next, { fromIndex, toIndex })
  }

  return (
    <div
      role="table"
      aria-label={`${currentPlaylist.name} tracks`}
      className="flex min-h-0 min-w-0 grow flex-col px-2 md:px-4 pb-2 md:pb-4"
    >
      {!hideHeader && (
        <div
          role="rowgroup"
          className="mb-2 shrink-0 rounded-md bg-sidebar px-1"
        >
          <div
            role="row"
            className={cn('grid h-12 md:h-9 items-center gap-2 px-2 text-[11px] font-medium tracking-wide text-muted-foreground uppercase md:gap-3 md:px-2.5', headerGridClass)}
          >
            {table.getHeaderGroups().map(headerGroup =>
              headerGroup.headers.map((header) => {
                const canSort = header.column.getCanSort()
                const sorted = header.column.getIsSorted()

                if (header.id === 'select' || header.id === 'drag') {
                  return (
                    <div
                      key={header.id}
                      role="columnheader"
                      className="flex size-full items-center justify-center"
                    >
                      {flexRender(
                        header.column.columnDef.header,
                        header.getContext(),
                      )}
                    </div>
                  )
                }

                return (
                  <div
                    key={header.id}
                    role="columnheader"
                    className={cn(
                      header.id === 'duration' && 'justify-self-end',
                      header.id === 'performer' && 'hidden md:block',
                    )}
                  >
                    {canSort
                      ? (
                          <button
                            type="button"
                            className={cn(
                              'inline-flex items-center gap-1 text-xs transition-colors hover:text-foreground',
                              sorted && 'text-foreground',
                            )}
                            onClick={header.column.getToggleSortingHandler()}
                          >
                            {flexRender(
                              header.column.columnDef.header,
                              header.getContext(),
                            )}
                            <SortIcon sorted={sorted} />
                          </button>
                        )
                      : (
                          flexRender(
                            header.column.columnDef.header,
                            header.getContext(),
                          )
                        )}
                  </div>
                )
              }),
            )}
          </div>
        </div>
      )}

      <ScrollArea
        className="min-h-0 grow"
        viewportRef={scrollRef}

        viewportStyle={{ overflowX: 'hidden' }}
      >
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          modifiers={[restrictToVerticalAxis]}
          onDragEnd={handleDragEnd}
        >
          <SortableContext
            items={sortableIds}
            strategy={verticalListSortingStrategy}
          >
            <div
              role="rowgroup"
              className="relative w-full"
              style={{ height: `${virtualizer.getTotalSize()}px` }}
            >
              {virtualizer.getVirtualItems().map((virtualRow) => {
                const row = rows[virtualRow.index]
                if (!row) return null

                const track = row.original
                const sourceIndex = Number(row.id)
                const isSelected = row.getIsSelected()
                const sortableId = rowSortableIds[row.index]
                if (!sortableId) return null
                const menuProps = {
                  track,
                  sourceIndex,
                  isLiked: isTrackLiked(track.id),
                  currentPlaylist,
                  customPlaylists,
                  onSelect: onEnterSelection,
                  ...trackActions,
                }

                return (
                  <PlaylistTrackContextMenu
                    disabled={selectionMode || touchScreen}
                    key={sortableId}
                    {...menuProps}
                  >
                    <PlaylistTrackRow
                      virtualStart={virtualRow.start}
                      track={track}
                      sortableId={sortableId}
                      isActive={playingSourceIndex === sourceIndex}
                      isPlaying={isPlaying}
                      isSelected={isSelected}
                      selectionMode={selectionMode}
                      touchScreen={touchScreen}
                      canReorder={canReorder}
                      onEnterSelection={() => onEnterSelection(sourceIndex)}
                      touchOptions={touchScreen
                        ? <PlaylistTrackDropdownMenu {...menuProps} />
                        : undefined}
                      onRowClick={(extend = false) => {
                        if (selectionMode) {
                          onTrackSelect(sourceIndex, !isSelected, extend)
                          return
                        }
                        onTrackPlay(track, sourceIndex)
                      }}
                      onToggleSelected={(selected, extend = false) => {
                        onTrackSelect(sourceIndex, selected, extend)
                      }}
                    />
                  </PlaylistTrackContextMenu>
                )
              })}
            </div>
          </SortableContext>
        </DndContext>
      </ScrollArea>
    </div>
  )
}
