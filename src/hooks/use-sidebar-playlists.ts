import { BUILTIN_PLAYLIST_IDS, BUILTIN_PLAYLISTS } from '@/lib/playlists'
import { errorMessage } from '@/lib/errors'
import { useEffect, useMemo, useState } from 'react'
import type { DragEndEvent } from '@dnd-kit/core'
import {
  KeyboardSensor,
  MouseSensor,
  TouchSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core'
import { sortableKeyboardCoordinates } from '@dnd-kit/sortable'
import type { CustomPlaylistSummary } from '@/lib/db'
import {
  ALL_TRACKS_PLAYLIST_ID,
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
  trackIds?: number[]
  playlist?: CustomPlaylistSummary
}

export function useSidebarPlaylists() {
  const library = useLibraryStore(state => state.library)
  const libraryTrackCount = library.length
  const statsByTrackId = useListenStatsStore(state => state.statsByTrackId)
  const statisticsEnabled = useListenStatsStore(state => state.enabled)
  const playlistsData = usePlaylistsStore(state => state.data)
  const selectedPlaylistId = usePlaylistsStore(
    state => state.selectedPlaylistId,
  )
  const setSelectedPlaylist = usePlaylistsStore(
    state => state.setSelectedPlaylist,
  )
  const deletePlaylist = usePlaylistsStore(state => state.deletePlaylist)
  const [actionError, setActionError] = useState<string | null>(null)

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

  useEffect(() => {
    if (
      !statisticsEnabled
      && (selectedPlaylistId === POPULAR_PLAYLIST_ID
        || selectedPlaylistId === RECENT_PLAYLIST_ID)
    ) {
      setSelectedPlaylist(ALL_TRACKS_PLAYLIST_ID)
    }
  }, [statisticsEnabled, selectedPlaylistId, setSelectedPlaylist])

  const smartCount = smartPlaylistTrackCount(library, statsByTrackId)
  const smartUpdatedAt = smartPlaylistUpdatedAt(library, statsByTrackId)
  const playlistItems: SidebarPlaylistListItem[] = [
    ...BUILTIN_PLAYLIST_IDS.flatMap((id): SidebarPlaylistListItem[] => {
      const definition = BUILTIN_PLAYLISTS[id]
      if (definition.smartSort && !statisticsEnabled) return []
      if (id === 'liked' && !playlistsData) return []
      return [{ id, name: definition.name, thumbnailVariant: id,
        count: id === 'all' ? libraryTrackCount : id === 'liked' ? playlistsData!.liked.trackIds.length : smartCount,
        updatedAt: id === 'all' ? libraryUpdatedAt(library) : id === 'liked' ? playlistsData!.liked.updatedAt : smartUpdatedAt,
      }]
    }),
    ...(playlistsData?.custom.map(playlist => ({
      id: playlist.id, name: playlist.name, count: playlist.trackIds.length, updatedAt: playlist.updatedAt,
      thumbnailVariant: 'custom' as const, trackIds: playlist.trackIds, playlist,
    })) ?? []),
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
      if (
        !statisticsEnabled
        && (id === POPULAR_PLAYLIST_ID || id === RECENT_PLAYLIST_ID)
      ) continue
      entries.push({ id, name: HIDEABLE_PLAYLIST_LABELS[id] })
    }
    entries.sort((a, b) => a.name.localeCompare(b.name, undefined, {
      sensitivity: 'base',
    }))
    return entries
  }, [hiddenPlaylists, statisticsEnabled])

  const canReorder = sortMode === 'custom' && search.length === 0

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
      await deletePlaylist(id)
      setActionError(null)
    }
    catch (error) {
      setActionError(errorMessage(error))
    }

    finally {
      setDeletingId(null)
    }
  }

  return {
    actionError,
    setActionError,
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
