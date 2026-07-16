import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  type DragStartEvent,
  DragOverlay,
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
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type OnChangeFn,
  type RowSelectionState,
  type SortingState,
} from '@tanstack/react-table'
import { useVirtualizer } from '@tanstack/react-virtual'
import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-react'
import { useMemo, useRef, useState } from 'react'
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
  PlaylistTrackRowView,
} from './PlaylistTrackRow'
import { compareTracks, reorderTrackIds, type CustomPlaylistRef } from './track-actions'

export interface PlaylistTracksTableProps {
  tracks: Track[]
  currentPlaylist: ResolvedSelectedPlaylist
  customPlaylists: CustomPlaylistRef[]
  currentTrackId: number | null
  isPlaying: boolean
  isTrackLiked: (trackId: number) => boolean
  selectionMode: boolean
  rowSelection: RowSelectionState
  onRowSelectionChange: OnChangeFn<RowSelectionState>
  sorting: SortingState
  onSortingChange: OnChangeFn<SortingState>
  canReorder: boolean
  onReorderTracks: (trackIds: number[]) => void
  onEnterSelection: (trackId: number) => void
  onTrackPlay: (track: Track, startIndex: number) => void
  onToggleLike: (trackId: number) => void
  onAddToPlaylist: (playlistId: number, trackId: number) => void
  onDeleteFromPlaylist: (playlistId: number, trackId: number) => void
  onDownload: (track: Track) => void
  onShowInfo: (track: Track) => void
}

function SortIcon({ sorted }: { sorted: false | 'asc' | 'desc' }) {
  if (sorted === 'asc') return <ArrowUp className="size-3.5 opacity-80" />
  if (sorted === 'desc') return <ArrowDown className="size-3.5 opacity-80" />
  return <ArrowUpDown className="size-3.5 opacity-40" />
}

export function PlaylistTracksTable({
  tracks,
  currentPlaylist,
  customPlaylists,
  currentTrackId,
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
  onDownload,
  onShowInfo,
}: PlaylistTracksTableProps) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const [activeTrackId, setActiveTrackId] = useState<number | null>(null)

  const columns = useMemo<ColumnDef<Track>[]>(() => {
    const defs: ColumnDef<Track>[] = []

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
        sortingFn: (rowA, rowB) =>
          compareTracks(rowA.original, rowB.original, {
            id: 'title',
            desc: false,
          }),
      },
      {
        accessorKey: 'performer',
        header: 'Artist',
        cell: () => null,
        sortingFn: (rowA, rowB) =>
          compareTracks(rowA.original, rowB.original, {
            id: 'performer',
            desc: false,
          }),
      },
      {
        accessorKey: 'duration',
        header: 'Time',
        cell: () => null,
        sortingFn: (rowA, rowB) =>
          compareTracks(rowA.original, rowB.original, {
            id: 'duration',
            desc: false,
          }),
      },
    )

    return defs
  }, [selectionMode])

  // TanStack Table / Virtual intentionally return live functions
  // eslint-disable-next-line react-hooks/incompatible-library
  const table = useReactTable({
    data: tracks,
    columns,
    state: {
      sorting,
      rowSelection,
    },
    getRowId: row => String(row.id),
    enableRowSelection: selectionMode,
    onSortingChange,
    onRowSelectionChange,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  })

  const rows = table.getRowModel().rows
  const sortableIds = useMemo(() => tracks.map(track => track.id), [tracks])

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 6 },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  )

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

  const activeTrack = activeTrackId === null
    ? null
    : tracks.find(track => track.id === activeTrackId) ?? null

  const handleDragStart = (event: DragStartEvent) => {
    setActiveTrackId(Number(event.active.id))
  }

  const handleDragCancel = () => {
    setActiveTrackId(null)
  }

  const handleDragEnd = (event: DragEndEvent) => {
    setActiveTrackId(null)
    if (!canReorder) return
    const { active, over } = event
    if (!over || active.id === over.id) return

    const next = reorderTrackIds(
      sortableIds,
      Number(active.id),
      Number(over.id),
    )
    if (next === sortableIds) return
    onReorderTracks(next)
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
          onDragStart={handleDragStart}
          onDragCancel={handleDragCancel}
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
                const isSelected = row.getIsSelected()

                return (
                  <PlaylistTrackContextMenu
                    key={row.id}
                    track={track}
                    isLiked={isTrackLiked(track.id)}
                    currentPlaylist={currentPlaylist}
                    customPlaylists={customPlaylists}
                    onSelect={onEnterSelection}
                    onToggleLike={onToggleLike}
                    onAddToPlaylist={onAddToPlaylist}
                    onDeleteFromPlaylist={onDeleteFromPlaylist}
                    onDownload={onDownload}
                    onShowInfo={onShowInfo}
                  >
                    <PlaylistTrackRow
                      virtualStart={virtualRow.start}
                      track={track}
                      isActive={currentTrackId === track.id}
                      isPlaying={isPlaying}
                      isSelected={isSelected}
                      selectionMode={selectionMode}
                      canReorder={canReorder}
                      onRowClick={() => {
                        if (selectionMode) {
                          row.toggleSelected(!isSelected)
                          return
                        }
                        onTrackPlay(track, virtualRow.index)
                      }}
                      onToggleSelected={(selected) => {
                        row.toggleSelected(selected)
                      }}
                      onPlayFromThumb={() => {
                        onTrackPlay(track, virtualRow.index)
                      }}
                    />
                  </PlaylistTrackContextMenu>
                )
              })}
            </div>
          </SortableContext>

          <DragOverlay dropAnimation={null}>
            {activeTrack
              ? (
                  <PlaylistTrackRowView
                    track={activeTrack}
                    isActive={currentTrackId === activeTrack.id}
                    isPlaying={isPlaying}
                    isSelected={false}
                    selectionMode={false}
                    className="cursor-grabbing bg-muted opacity-95 shadow-md"
                  />
                )
              : null}
          </DragOverlay>
        </DndContext>
      </div>
    </div>
  )
}
