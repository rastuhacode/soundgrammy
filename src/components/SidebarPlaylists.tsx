import { useEffect, useMemo, useState } from 'react'
import {
  ArrowUpDown,
  Check,
  Ellipsis,
  List,
  Pencil,
  Plus,
  Search,
  Trash2,
  X,
} from 'lucide-react'
import {
  DndContext,
  closestCenter,
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
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { PlaylistFormDialog } from '@/components/playlist/PlaylistFormDialog'
import { SidebarPlaylistThumbnail } from '@/components/playlist/SidebarPlaylistThumbnail'
import type { CustomPlaylistSummary } from '@/lib/db'
import {
  ALL_TRACKS_PLAYLIST_ID,
  LIKED_PLAYLIST_ID,
  type PlaylistId,
  usePlaylistsStore,
} from '@/stores/playlists-store'
import { useLibraryStore } from '@/stores/library-store'
import { api } from '@/lib/api'
import { cn } from '@/lib/utils'
import { useFilter } from '@/hooks/utils/use-filter'
import {
  libraryUpdatedAt,
  PLAYLIST_SORT_MODE_LABELS,
  readCustomOrder,
  readSortMode,
  readSortReversed,
  reconcileCustomOrder,
  reorderPlaylistIds,
  sortPlaylistItems,
  writeCustomOrder,
  writeSortMode,
  writeSortReversed,
  type PlaylistSortMode,
} from '@/lib/playlist-sort'
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from './ui/input-group'

type ThumbnailVariant
  = | typeof ALL_TRACKS_PLAYLIST_ID
    | typeof LIKED_PLAYLIST_ID
    | 'custom'

interface PlaylistListItem {
  id: PlaylistId
  name: string
  count: number
  updatedAt: string
  thumbnailVariant: ThumbnailVariant
  playlistId?: number
  hasThumbnail?: boolean
  playlist?: CustomPlaylistSummary
}

interface PlaylistItemProps {
  id: PlaylistId
  name: string
  count: number
  isActive: boolean
  thumbnailVariant: ThumbnailVariant
  playlistId?: number
  hasThumbnail?: boolean
  onSelect: () => void
  onEdit?: () => void
  onDelete?: () => void
  isDeleting?: boolean
  sortable?: boolean
}

function PlaylistItem({
  id,
  name,
  count,
  isActive,
  thumbnailVariant,
  playlistId,
  hasThumbnail,
  onSelect,
  onEdit,
  onDelete,
  isDeleting,
  sortable = false,
}: PlaylistItemProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id,
    disabled: !sortable,
  })

  return (
    <div
      ref={setNodeRef}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
      }}
      className={cn(
        'group flex w-full items-center gap-3 rounded-lg px-2 py-2 transition-colors',
        isActive
          ? 'bg-accent text-accent-foreground'
          : 'text-foreground hover:bg-muted/70',
        sortable && 'cursor-grab active:cursor-grabbing',
        isDragging && 'z-10 bg-muted opacity-90 shadow-md',
      )}
      role="button"
      aria-label={`Select ${name} playlist`}
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onSelect()
        }
      }}
      {...(sortable ? { ...attributes, ...listeners } : {})}
    >
      <SidebarPlaylistThumbnail
        variant={thumbnailVariant}
        playlistId={playlistId}
        hasThumbnail={hasThumbnail}
        name={name}
      />

      <div className="min-w-0 grow">
        <p
          className={cn(
            'truncate text-sm font-medium',
            isActive ? 'text-foreground' : 'text-foreground/90',
          )}
        >
          {name}
        </p>
      </div>

      <div className="flex shrink-0 items-center gap-1">
        <span className="min-w-6 text-right font-mono text-xs tabular-nums text-muted-foreground">
          {count}
        </span>

        <div className="flex size-6 shrink-0 items-center justify-center">
          {onEdit || onDelete
            ? (
                <DropdownMenu>
                  <DropdownMenuTrigger render={(
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-xs"
                      aria-label={`${name} options`}
                      className="text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
                      onClick={e => e.stopPropagation()}
                    >
                      <Ellipsis className="size-4" />
                    </Button>
                  )}
                  />
                  <DropdownMenuContent align="end" className="w-40" onClick={e => e.stopPropagation()}>
                    {onEdit
                      ? (
                          <DropdownMenuItem onClick={onEdit}>
                            <Pencil className="size-4" />
                            Edit playlist
                          </DropdownMenuItem>
                        )
                      : null}
                    {onDelete
                      ? (
                          <DropdownMenuItem
                            variant="destructive"
                            disabled={isDeleting}
                            onClick={onDelete}
                          >
                            <Trash2 className="size-4" />
                            Delete playlist
                          </DropdownMenuItem>
                        )
                      : null}
                  </DropdownMenuContent>
                </DropdownMenu>
              )
            : null}
        </div>
      </div>
    </div>
  )
}

type DialogState
  = | { mode: 'create' }
    | { mode: 'edit', playlist: CustomPlaylistSummary }

const SORT_MODES: PlaylistSortMode[] = ['recency', 'custom', 'alphabetical']

export function SidebarPlaylists() {
  const library = useLibraryStore(state => state.library)
  const libraryTrackCount = library.length
  const playlistsData = usePlaylistsStore(state => state.data)
  const selectedPlaylistId = usePlaylistsStore(
    state => state.selectedPlaylistId,
  )
  const setSelectedPlaylist = usePlaylistsStore(
    state => state.setSelectedPlaylist,
  )
  const setData = usePlaylistsStore(state => state.setData)

  const [dialogState, setDialogState] = useState<DialogState | null>(null)
  const [deletingId, setDeletingId] = useState<number | null>(null)
  const [search, setSearch] = useState('')
  const [sortMode, setSortMode] = useState<PlaylistSortMode>(() => readSortMode())
  const [sortReversed, setSortReversed] = useState(() => readSortReversed())
  const [persistedOrder, setPersistedOrder] = useState<PlaylistId[] | null>(() =>
    readCustomOrder(),
  )
  const { contains } = useFilter()

  const customs = playlistsData?.custom
  const customIds = useMemo(
    () => customs?.map(playlist => playlist.id) ?? [],
    [customs],
  )
  const customOrder = useMemo(
    () => reconcileCustomOrder(persistedOrder, customIds),
    [persistedOrder, customIds],
  )

  useEffect(() => {
    writeCustomOrder(customOrder)
  }, [customOrder])

  const playlistItems: PlaylistListItem[] = [
    {
      id: ALL_TRACKS_PLAYLIST_ID,
      name: 'All tracks',
      count: libraryTrackCount,
      updatedAt: libraryUpdatedAt(library),
      thumbnailVariant: ALL_TRACKS_PLAYLIST_ID,
    },
    ...(playlistsData
      ? [
          {
            id: LIKED_PLAYLIST_ID,
            name: 'Liked',
            count: playlistsData.liked.trackIds.length,
            updatedAt: playlistsData.liked.updatedAt,
            thumbnailVariant: LIKED_PLAYLIST_ID,
          } satisfies PlaylistListItem,
          ...playlistsData.custom.map(playlist => ({
            id: playlist.id,
            name: playlist.name,
            count: playlist.trackIds.length,
            updatedAt: playlist.updatedAt,
            thumbnailVariant: 'custom' as const,
            playlistId: playlist.id,
            hasThumbnail: playlist.hasThumbnail,
            playlist,
          })),
        ]
      : []),
  ]

  const filteredPlaylists = sortPlaylistItems(
    playlistItems.filter(playlist => contains(playlist.name, search)),
    sortMode,
    sortReversed,
    customOrder,
  )

  const canReorder = sortMode === 'custom' && search.length === 0

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 6 },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  )

  const handleSortModeChange = (mode: PlaylistSortMode) => {
    setSortMode(mode)
    writeSortMode(mode)
  }

  const handleReverseToggle = () => {
    if (sortMode === 'custom') return
    const next = !sortReversed
    setSortReversed(next)
    writeSortReversed(next)
  }

  const handleDragEnd = (event: DragEndEvent) => {
    if (!canReorder) return
    const { active, over } = event
    if (!over || active.id === over.id) return

    const activeId = active.id as PlaylistId
    const overId = over.id as PlaylistId
    const next = reorderPlaylistIds(customOrder, activeId, overId)
    setPersistedOrder(next)
    writeCustomOrder(next)
  }

  const handleDelete = async (id: number) => {
    if (!playlistsData) return
    setDeletingId(id)
    try {
      await api.deletePlaylist(id)
      setData({
        ...playlistsData,
        custom: playlistsData.custom.filter(playlist => playlist.id !== id),
      })
    }
    catch {
      // keep list unchanged on failure
    }
    finally {
      setDeletingId(null)
    }
  }

  const showReverseIcon = sortReversed && sortMode !== 'custom'

  return (
    <div className="flex min-h-0 grow flex-col gap-4">
      <div className="flex items-center justify-between gap-2 px-4">
        <h2 className="text-lg font-semibold tracking-tight text-foreground">
          Library
        </h2>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => setDialogState({ mode: 'create' })}
          aria-label="Create playlist"
          className="text-muted-foreground hover:text-foreground"
        >
          <Plus />
        </Button>
      </div>
      <div className="flex items-center gap-2 px-4">
        <div className="grow">
          <InputGroup>
            <InputGroupInput value={search} onChange={e => setSearch(e.target.value)} placeholder="Search" />
            <InputGroupAddon>
              <Search className="size-4" />
            </InputGroupAddon>
            {search.length > 0 && (
              <InputGroupButton onClick={() => setSearch('')}>
                <X className="size-4" />
              </InputGroupButton>
            )}
          </InputGroup>
        </div>

        <Popover>
          <PopoverTrigger
            render={(
              <Button
                aria-label="Sort playlists"
                variant="ghost"
                className="shrink-0 text-muted-foreground"
              >
                {PLAYLIST_SORT_MODE_LABELS[sortMode]}
                {showReverseIcon
                  ? <ArrowUpDown className="size-3.5" />
                  : <List className="size-4" />}
              </Button>
            )}
          />
          <PopoverContent align="end" className="w-52 gap-1 p-1.5">
            {SORT_MODES.map(mode => (
              <button
                key={mode}
                type="button"
                className={cn(
                  'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-hidden transition-colors hover:bg-accent hover:text-accent-foreground',
                  sortMode === mode && 'bg-accent/60',
                )}
                onClick={() => handleSortModeChange(mode)}
              >
                <span className="grow">{PLAYLIST_SORT_MODE_LABELS[mode]}</span>
                {sortMode === mode
                  ? <Check className="size-4 shrink-0" />
                  : <span className="size-4 shrink-0" />}
              </button>
            ))}
            <div className="my-1 h-px bg-border" />
            <button
              type="button"
              disabled={sortMode === 'custom'}
              className={cn(
                'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-hidden transition-colors',
                sortMode === 'custom'
                  ? 'cursor-not-allowed text-muted-foreground opacity-50'
                  : 'hover:bg-accent hover:text-accent-foreground',
                sortReversed && sortMode !== 'custom' && 'bg-accent/60',
              )}
              onClick={handleReverseToggle}
            >
              <ArrowUpDown className="size-4 shrink-0" />
              <span className="grow">Reverse</span>
              {sortReversed && sortMode !== 'custom'
                ? <Check className="size-4 shrink-0" />
                : <span className="size-4 shrink-0" />}
            </button>
          </PopoverContent>
        </Popover>
      </div>

      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragEnd={handleDragEnd}
      >
        <SortableContext
          items={filteredPlaylists.map(item => item.id)}
          strategy={verticalListSortingStrategy}
        >
          <div className="flex min-h-0 grow flex-col gap-0.5 overflow-y-auto px-2 pb-2">
            {filteredPlaylists.map((item) => {
              const customPlaylist = item.playlist

              return (
                <PlaylistItem
                  key={String(item.id)}
                  id={item.id}
                  name={item.name}
                  count={item.count}
                  isActive={selectedPlaylistId === item.id}
                  thumbnailVariant={item.thumbnailVariant}
                  playlistId={item.playlistId}
                  hasThumbnail={item.hasThumbnail}
                  sortable={canReorder}
                  onSelect={() => setSelectedPlaylist(item.id)}
                  onEdit={customPlaylist
                    ? () => setDialogState({ mode: 'edit', playlist: customPlaylist })
                    : undefined}
                  onDelete={customPlaylist
                    ? () => handleDelete(customPlaylist.id)
                    : undefined}
                  isDeleting={customPlaylist ? deletingId === customPlaylist.id : undefined}
                />
              )
            })}
          </div>
        </SortableContext>
      </DndContext>

      <PlaylistFormDialog
        open={dialogState !== null}
        onOpenChange={(open) => {
          if (!open) setDialogState(null)
        }}
        mode={dialogState?.mode ?? 'create'}
        playlist={dialogState?.mode === 'edit' ? dialogState.playlist : undefined}
      />
    </div>
  )
}
