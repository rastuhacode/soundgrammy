import { useEffect, useMemo, useState } from 'react'
import type { DragEndEvent } from '@dnd-kit/core'
import {
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core'
import { sortableKeyboardCoordinates } from '@dnd-kit/sortable'
import type { CustomPlaylistSummary } from '@/lib/db'
import {
  ALL_TRACKS_PLAYLIST_ID,
  LIKED_PLAYLIST_ID,
  POPULAR_PLAYLIST_ID,
  RECENT_PLAYLIST_ID,
  type PlaylistId,
  usePlaylistsStore,
} from '@/stores/playlists-store'
import { useLibraryStore } from '@/stores/library-store'
import {
  smartPlaylistTrackCount,
  smartPlaylistUpdatedAt,
  useListenStatsStore,
} from '@/stores/listen-stats-store'
import { api } from '@/lib/api'
import { useFilter } from '@/hooks/utils/use-filter'
import {
  libraryUpdatedAt,
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
import {
  canHidePlaylist,
  HIDEABLE_PLAYLIST_LABELS,
  type HideablePlaylistId,
  readHiddenPlaylists,
  writeHiddenPlaylists,
} from '@/lib/playlist-visibility'
import type { SidebarPlaylistThumbnailVariant } from '@/components/playlist/SidebarPlaylistThumbnail'

export type SidebarPlaylistDialogState
  = | { mode: 'create' }
    | { mode: 'edit', playlist: CustomPlaylistSummary }

export interface SidebarPlaylistListItem {
  id: PlaylistId
  name: string
  count: number
  updatedAt: string
  thumbnailVariant: SidebarPlaylistThumbnailVariant
  playlistId?: number
  hasThumbnail?: boolean
  playlist?: CustomPlaylistSummary
}

export function useSidebarPlaylists() {
  const library = useLibraryStore(state => state.library)
  const libraryTrackCount = library.length
  const statsByTrackId = useListenStatsStore(state => state.statsByTrackId)
  const playlistsData = usePlaylistsStore(state => state.data)
  const selectedPlaylistId = usePlaylistsStore(
    state => state.selectedPlaylistId,
  )
  const setSelectedPlaylist = usePlaylistsStore(
    state => state.setSelectedPlaylist,
  )
  const setData = usePlaylistsStore(state => state.setData)

  const [dialogState, setDialogState] = useState<SidebarPlaylistDialogState | null>(null)
  const [deletingId, setDeletingId] = useState<number | null>(null)
  const [search, setSearch] = useState('')
  const [sortMode, setSortMode] = useState<PlaylistSortMode>(() => readSortMode())
  const [sortReversed, setSortReversed] = useState(() => readSortReversed())
  const [persistedOrder, setPersistedOrder] = useState<PlaylistId[] | null>(() =>
    readCustomOrder(),
  )
  const [hiddenPlaylists, setHiddenPlaylists] = useState(() => readHiddenPlaylists())
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

  const smartCount = smartPlaylistTrackCount(library, statsByTrackId)
  const smartUpdatedAt = smartPlaylistUpdatedAt(library, statsByTrackId)

  const playlistItems: SidebarPlaylistListItem[] = [
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
          } satisfies SidebarPlaylistListItem,
        ]
      : []),
    {
      id: POPULAR_PLAYLIST_ID,
      name: 'Popular',
      count: smartCount,
      updatedAt: smartUpdatedAt,
      thumbnailVariant: POPULAR_PLAYLIST_ID,
    },
    {
      id: RECENT_PLAYLIST_ID,
      name: 'Recent',
      count: smartCount,
      updatedAt: smartUpdatedAt,
      thumbnailVariant: RECENT_PLAYLIST_ID,
    },
    ...(playlistsData
      ? playlistsData.custom.map(playlist => ({
          id: playlist.id,
          name: playlist.name,
          count: playlist.trackIds.length,
          updatedAt: playlist.updatedAt,
          thumbnailVariant: 'custom' as const,
          playlistId: playlist.id,
          hasThumbnail: playlist.hasThumbnail,
          playlist,
        }))
      : []),
  ]

  const visiblePlaylists = playlistItems.filter((playlist) => {
    if (!canHidePlaylist(playlist.id)) return true
    return !hiddenPlaylists.has(playlist.id)
  })

  const filteredPlaylists = sortPlaylistItems(
    visiblePlaylists.filter(playlist => contains(playlist.name, search)),
    sortMode,
    sortReversed,
    customOrder,
  )

  const hiddenEntries = useMemo(() => {
    const entries: Array<{ id: HideablePlaylistId, name: string }> = []
    for (const id of hiddenPlaylists) {
      entries.push({ id, name: HIDEABLE_PLAYLIST_LABELS[id] })
    }
    entries.sort((a, b) => a.name.localeCompare(b.name, undefined, {
      sensitivity: 'base',
    }))
    return entries
  }, [hiddenPlaylists])

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

  const handleSortReversedChange = (reversed: boolean) => {
    setSortReversed(reversed)
    writeSortReversed(reversed)
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

  const handleHide = (id: HideablePlaylistId) => {
    setHiddenPlaylists((prev) => {
      const next = new Set(prev)
      next.add(id)
      writeHiddenPlaylists(next)
      return next
    })
    if (selectedPlaylistId === id) {
      setSelectedPlaylist(ALL_TRACKS_PLAYLIST_ID)
    }
  }

  const handleUnhide = (id: HideablePlaylistId) => {
    setHiddenPlaylists((prev) => {
      const next = new Set(prev)
      next.delete(id)
      writeHiddenPlaylists(next)
      return next
    })
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

  return {
    selectedPlaylistId,
    setSelectedPlaylist,
    dialogState,
    setDialogState,
    deletingId,
    search,
    setSearch,
    sortMode,
    sortReversed,
    hiddenEntries,
    filteredPlaylists,
    canReorder,
    sensors,
    handleSortModeChange,
    handleSortReversedChange,
    handleDragEnd,
    handleHide,
    handleUnhide,
    handleDelete,
  }
}
