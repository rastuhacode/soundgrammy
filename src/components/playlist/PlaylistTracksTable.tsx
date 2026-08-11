import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  type Modifier,
  KeyboardSensor,
  PointerSensor,
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
  createSortedRowModel,
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
import { useMemo, useRef } from 'react'
import type { Track } from '@/lib/db'
import type { ResolvedSelectedPlaylist } from '@/stores/playlists-store'
import { cn } from '@/lib/utils'
import { Checkbox } from '@/components/ui/checkbox'
import { PlaylistTrackContextMenu } from './PlaylistTrackContextMenu'
import {
  TRACK_GRID_COLS,
  TRACK_GRID_COLS_SELECT,
  TRACK_ROW_STRIDE,
  PlaylistTrackRow,
} from './PlaylistTrackRow'
import {
  compareTracks,
  getTrackSortableIds,
  reorderByIndex,
  type CustomPlaylistRef,
} from './track-actions'

const playlistTableFeatures = tableFeatures({
  columnSizingFeature,
  rowSelectionFeature,
  rowSortingFeature,
  sortedRowModel: createSortedRowModel(),
})

const restrictToVerticalAxis: Modifier = ({ transform }) => ({
  ...transform,
  x: 0,
})

export interface PlaylistTracksTableProps {
  tracks: Track[]
  sourceIndices: number[]
  currentPlaylist: ResolvedSelectedPlaylist
  customPlaylists: CustomPlaylistRef[]
  /** Membership index of the now-playing row; null when nothing should highlight. */
  playingSourceIndex: number | null
  isPlaying: boolean
  isTrackLiked: (trackId: number) => boolean
  selectionMode: boolean
  rowSelection: RowSelectionState
  onRowSelectionChange: OnChangeFn<RowSelectionState>
  sorting: SortingState
  onSortingChange: OnChangeFn<SortingState>
  canReorder: boolean
  onReorderTracks: (
    trackIds: number[],
    move: { fromIndex: number, toIndex: number },
  ) => void
  onEnterSelection: (sourceIndex: number) => void
  onTrackPlay: (track: Track, startIndex: number) => void
  onToggleLike: (trackId: number) => void
  onAddToPlaylist: (playlistId: number, trackId: number) => void
  onDeleteFromPlaylist: (playlistId: number, position: number) => void
  onPlayNext: (track: Track) => void
  onAddToEnd: (track: Track) => void
  onCache: (track: Track) => void
  onDownload: (track: Track) => void
  onRemoveFromCache: (track: Track) => void
  onShowInfo: (track: Track) => void
}

function SortIcon({ sorted }: { sorted: false | 'asc' | 'desc' }) {
  if (sorted === 'asc') return <ArrowUp className="size-3.5 opacity-80" />
  if (sorted === 'desc') return <ArrowDown className="size-3.5 opacity-80" />
  return <ArrowUpDown className="size-3.5 opacity-40" />
}

export function PlaylistTracksTable({
  tracks,
  sourceIndices,
  currentPlaylist,
  customPlaylists,
  playingSourceIndex,
  isPlaying,
  isTrackLiked,
  selectionMode,
  rowSelection,
  onRowSelectionChange,
  sorting,
  onSortingChange,
  canReorder,
  onReorderTracks,
  onEnterSelection,
  onTrackPlay,
  onToggleLike,
  onAddToPlaylist,
  onDeleteFromPlaylist,
  onPlayNext,
  onAddToEnd,
  onCache,
  onDownload,
  onRemoveFromCache,
  onShowInfo,
}: PlaylistTracksTableProps) {
  const scrollRef = useRef<HTMLDivElement>(null)

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

    defs.push(
      {
        accessorKey: 'title',
        header: 'Title',
        cell: () => null,
        sortFn: (rowA, rowB) =>
          compareTracks(rowA.original, rowB.original, {
            id: 'title',
            desc: false,
          }),
      },
      {
        accessorKey: 'performer',
        header: 'Artist',
        cell: () => null,
        sortFn: (rowA, rowB) =>
          compareTracks(rowA.original, rowB.original, {
            id: 'performer',
            desc: false,
          }),
      },
      {
        accessorKey: 'duration',
        header: 'Time',
        cell: () => null,
        sortFn: (rowA, rowB) =>
          compareTracks(rowA.original, rowB.original, {
            id: 'duration',
            desc: false,
          }),
      },
      {
        id: 'actions',
        header: () => <span className="sr-only">Track options</span>,
        cell: () => null,
        enableSorting: false,
      },
    )

    return defs
  }, [selectionMode])

  const table = useTable({
    features: playlistTableFeatures,
    data: tracks,
    columns,
    state: {
      sorting,
      rowSelection,
    },
    getRowId: (_row, index) => String(sourceIndices[index] ?? index),
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
    useSensor(PointerSensor, {
      activationConstraint: { distance: 6 },
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
    estimateSize: () => TRACK_ROW_STRIDE,
    overscan: 8,
  })

  const headerGridClass = selectionMode
    ? TRACK_GRID_COLS_SELECT
    : TRACK_GRID_COLS

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
    <div ref={scrollRef} className="min-h-0 grow overflow-y-auto px-4 pb-4">
      <div
        role="table"
        aria-label={`${currentPlaylist.name} tracks`}
        className="w-full"
      >
        <div
          role="rowgroup"
          className="sticky top-0 z-10 -mx-1 mb-2 rounded-md bg-sidebar px-1"
        >
          <div
            role="row"
            className="grid h-9 items-center gap-3 px-2.5 text-[11px] font-medium tracking-wide text-muted-foreground uppercase"
            style={{ gridTemplateColumns: headerGridClass }}
          >
            {table.getHeaderGroups().map(headerGroup =>
              headerGroup.headers.map((header) => {
                const canSort = header.column.getCanSort()
                const sorted = header.column.getIsSorted()

                if (header.id === 'select') {
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

                return (
                  <PlaylistTrackContextMenu
                    disabled={selectionMode}
                    key={sortableId}
                    track={track}
                    sourceIndex={sourceIndex}
                    isLiked={isTrackLiked(track.id)}
                    currentPlaylist={currentPlaylist}
                    customPlaylists={customPlaylists}
                    onSelect={onEnterSelection}
                    onToggleLike={onToggleLike}
                    onAddToPlaylist={onAddToPlaylist}
                    onDeleteFromPlaylist={onDeleteFromPlaylist}
                    onPlayNext={onPlayNext}
                    onAddToEnd={onAddToEnd}
                    onCache={onCache}
                    onDownload={onDownload}
                    onRemoveFromCache={onRemoveFromCache}
                    onShowInfo={onShowInfo}
                  >
                    <PlaylistTrackRow
                      virtualStart={virtualRow.start}
                      track={track}
                      sortableId={sortableId}
                      isActive={playingSourceIndex === sourceIndex}
                      isPlaying={isPlaying}
                      isSelected={isSelected}
                      selectionMode={selectionMode}
                      canReorder={canReorder}
                      onRowClick={() => {
                        if (selectionMode) {
                          row.toggleSelected(!isSelected)
                          return
                        }
                        onTrackPlay(track, sourceIndex)
                      }}
                      onToggleSelected={(selected) => {
                        row.toggleSelected(selected)
                      }}
                    />
                  </PlaylistTrackContextMenu>
                )
              })}
            </div>
          </SortableContext>
        </DndContext>
      </div>
    </div>
  )
}
