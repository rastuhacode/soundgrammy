import type { Track } from '@/lib/db'
import { useLibraryStore } from '@/stores/library-store'
import { usePlayerStore } from '@/stores/player-store'
import {
  ALL_TRACKS_PLAYLIST_ID,
  getLikedTrackIdSet,
  isTrackLiked,
  LIKED_PLAYLIST_ID,
  resolveSelectedPlaylistTracks,
  usePlaylistsStore,
} from '@/stores/playlists-store'
import type {
  CustomPlaylistId,
  ResolvedSelectedPlaylist,
} from '@/stores/playlists-store'
import { Music, Play, Search, Shuffle, Undo2, X } from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import { useMemo, useState } from 'react'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import type { RowSelectionState, SortingState } from '@tanstack/react-table'
import { Separator } from '@/components/ui/separator'
import { api } from '@/lib/api'

import { PlaylistBulkActions } from './PlaylistBulkActions'
import { PlaylistTracksTable } from './PlaylistTracksTable'
import { TrackInfoDialog } from './TrackInfoDialog'
import {
  enterSelectionWithTrack,
  sortingStateToTrackSort,
  sortTracks,
  toPlayablePlaylist,
} from './track-actions'
import { Button } from '@/components/ui/button'
import { useFilter } from '@/hooks/utils/use-filter'
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from '@/components/ui/input-group'

function getEmptyStateCopy(
  libraryTrackCount: number,
  playlistId: ResolvedSelectedPlaylist['id'],
  isCustom: boolean,
): { title: string, description: string } {
  if (libraryTrackCount === 0) {
    return {
      title: 'No tracks yet',
      description:
        'Pin music to your Telegram profile and it will tune in here automatically.',
    }
  }

  if (playlistId === LIKED_PLAYLIST_ID) {
    return {
      title: 'No liked tracks yet',
      description: 'Tap the heart on any track to save it here.',
    }
  }

  if (isCustom) {
    return {
      title: 'This playlist is empty',
      description: 'Add tracks from your library using the list button.',
    }
  }

  return {
    title: 'No tracks yet',
    description:
      'Pin music to your Telegram profile and it will tune in here automatically.',
  }
}

export function PlaylistView() {
  const selectedPlaylistId = usePlaylistsStore(
    state => state.selectedPlaylistId,
  )

  // Remount on playlist change so search / selection reset without an effect.
  return <PlaylistViewContent key={selectedPlaylistId} />
}

function PlaylistViewContent() {
  const [search, setSearch] = useState('')
  const [selectionMode, setSelectionMode] = useState(false)
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({})
  const [sorting, setSorting] = useState<SortingState>([])
  const [infoTrack, setInfoTrack] = useState<Track | null>(null)

  const { contains } = useFilter()

  const libraryTracks = useLibraryStore(state => state.library)
  const currentTrackId = usePlayerStore(
    state => state.currentTrack?.id ?? null,
  )
  const isPlaying = usePlayerStore(state => state.isPlaying)
  const playPlaylist = usePlayerStore(state => state.playPlaylist)
  const data = usePlaylistsStore(state => state.data)
  const selectedPlaylistId = usePlaylistsStore(
    state => state.selectedPlaylistId,
  )
  const setData = usePlaylistsStore(state => state.setData)

  const selectedPlaylist = useMemo(
    () => resolveSelectedPlaylistTracks(libraryTracks, data, selectedPlaylistId),
    [libraryTracks, data, selectedPlaylistId],
  )
  const {
    tracks: playlistTracks,
    isCustom,
    id: playlistId,
  } = selectedPlaylist

  const filteredTracks = useMemo(
    () =>
      playlistTracks.filter(track =>
        contains(`${track.performer} - ${track.title}`, search),
      ),
    [playlistTracks, contains, search],
  )

  const orderedTracks = useMemo(
    () => sortTracks(filteredTracks, sortingStateToTrackSort(sorting)),
    [filteredTracks, sorting],
  )

  const playablePlaylist = useMemo(
    () => toPlayablePlaylist(selectedPlaylist, orderedTracks),
    [selectedPlaylist, orderedTracks],
  )

  const likedTrackIds = useMemo(() => getLikedTrackIdSet(data), [data])
  const selectedTrackIds = useMemo(
    () =>
      Object.keys(rowSelection)
        .filter(id => rowSelection[id])
        .map(Number)
        .filter(id => Number.isFinite(id)),
    [rowSelection],
  )

  const canReorder
    = selectedPlaylistId !== ALL_TRACKS_PLAYLIST_ID
      && search.length === 0
      && sorting.length === 0
      && !selectionMode

  const handleTrackSelect = (track: Track, startIndex: number) => {
    playPlaylist(playablePlaylist, { start: track, startIndex })
  }

  const handleReorderTracks = async (trackIds: number[]) => {
    const latest = usePlaylistsStore.getState().data
    if (!latest) return

    const dbPlaylistId
      = selectedPlaylistId === LIKED_PLAYLIST_ID
        ? latest.liked.id
        : typeof selectedPlaylistId === 'number'
          ? selectedPlaylistId
          : null
    if (dbPlaylistId === null) return

    const previousTrackIds
      = selectedPlaylistId === LIKED_PLAYLIST_ID
        ? latest.liked.trackIds
        : latest.custom.find(playlist => playlist.id === selectedPlaylistId)
          ?.trackIds
    if (!previousTrackIds) return

    const trackIdsMatch = (current: number[]) =>
      current.length === trackIds.length
      && current.every((id, index) => id === trackIds[index])

    if (selectedPlaylistId === LIKED_PLAYLIST_ID) {
      setData({
        ...latest,
        liked: { ...latest.liked, trackIds },
      })
    }
    else {
      setData({
        ...latest,
        custom: latest.custom.map(playlist =>
          playlist.id === selectedPlaylistId
            ? { ...playlist, trackIds }
            : playlist,
        ),
      })
    }

    try {
      const updatedAt = await api.reorderPlaylistTracks(dbPlaylistId, trackIds)
      const after = usePlaylistsStore.getState().data
      if (!after) return
      // Only stamp updatedAt if this playlist still has our optimistic order.
      if (selectedPlaylistId === LIKED_PLAYLIST_ID) {
        if (!trackIdsMatch(after.liked.trackIds)) return
        setData({
          ...after,
          liked: { ...after.liked, updatedAt },
        })
      }
      else {
        const current = after.custom.find(
          playlist => playlist.id === selectedPlaylistId,
        )
        if (!current || !trackIdsMatch(current.trackIds)) return
        setData({
          ...after,
          custom: after.custom.map(playlist =>
            playlist.id === selectedPlaylistId
              ? { ...playlist, updatedAt }
              : playlist,
          ),
        })
      }
    }
    catch {
      const after = usePlaylistsStore.getState().data
      if (!after) return
      // Roll back only this playlist, and only if nothing else changed its order.
      if (selectedPlaylistId === LIKED_PLAYLIST_ID) {
        if (!trackIdsMatch(after.liked.trackIds)) return
        setData({
          ...after,
          liked: { ...after.liked, trackIds: previousTrackIds },
        })
      }
      else {
        const current = after.custom.find(
          playlist => playlist.id === selectedPlaylistId,
        )
        if (!current || !trackIdsMatch(current.trackIds)) return
        setData({
          ...after,
          custom: after.custom.map(playlist =>
            playlist.id === selectedPlaylistId
              ? { ...playlist, trackIds: previousTrackIds }
              : playlist,
          ),
        })
      }
    }
  }

  const handleEnterSelection = (trackId: number) => {
    const next = enterSelectionWithTrack(trackId)
    setSelectionMode(next.selectionMode)
    setRowSelection(next.rowSelection)
  }

  const handleExitSelection = () => {
    setSelectionMode(false)
    setRowSelection({})
  }

  const handleToggleLike = async (trackId: number) => {
    if (!usePlaylistsStore.getState().data) return
    try {
      const liked = await api.toggleLike(trackId)
      const latest = usePlaylistsStore.getState().data
      if (!latest) return
      setData({ ...latest, liked })
    }
    catch {
      // keep UI unchanged on failure
    }
  }

  const handleBulkAddToLiked = async (trackIds: number[]) => {
    for (const trackId of trackIds) {
      await handleToggleLike(trackId)
    }
  }

  const handleBulkRemoveFromLiked = async (trackIds: number[]) => {
    for (const trackId of trackIds) {
      await handleToggleLike(trackId)
    }
    setRowSelection({})
  }

  const handleAddToPlaylist = async (targetId: number, trackId: number) => {
    if (!usePlaylistsStore.getState().data) return
    try {
      const updatedAt = await api.addTrackToPlaylist(targetId, trackId)
      const latest = usePlaylistsStore.getState().data
      if (!latest) return
      setData({
        ...latest,
        custom: latest.custom.map(playlist =>
          playlist.id === targetId
            ? {
                ...playlist,
                updatedAt,
                trackIds: playlist.trackIds.includes(trackId)
                  ? playlist.trackIds
                  : [...playlist.trackIds, trackId],
              }
            : playlist,
        ),
      })
    }
    catch {
      // keep UI unchanged on failure
    }
  }

  const handleBulkAddToPlaylist = async (
    targetId: number,
    trackIds: number[],
  ) => {
    for (const trackId of trackIds) {
      await handleAddToPlaylist(targetId, trackId)
    }
  }

  const handleDeleteFromPlaylist = async (
    targetId: CustomPlaylistId,
    trackId: number,
  ) => {
    if (!usePlaylistsStore.getState().data) return
    try {
      const updatedAt = await api.removeTrackFromPlaylist(targetId, trackId)
      const latest = usePlaylistsStore.getState().data
      if (!latest) return
      setData({
        ...latest,
        custom: latest.custom.map(playlist =>
          playlist.id === targetId
            ? {
                ...playlist,
                updatedAt,
                trackIds: playlist.trackIds.filter(id => id !== trackId),
              }
            : playlist,
        ),
      })
    }
    catch {
      // keep UI unchanged on failure
    }
  }

  const handleBulkRemoveFromPlaylist = async (
    targetId: number,
    trackIds: number[],
  ) => {
    for (const trackId of trackIds) {
      await handleDeleteFromPlaylist(targetId, trackId)
    }
    setRowSelection({})
  }

  const handleDownload = async (track: Track) => {
    try {
      const path = await api.downloadTrack(track.id)
      await revealItemInDir(path)
    }
    catch {
      // ignore — file may still be downloading
    }
  }

  const handleBulkDownload = async (trackIds: number[]) => {
    const byId = new Map(playlistTracks.map(track => [track.id, track]))
    for (const trackId of trackIds) {
      const track = byId.get(trackId)
      if (track) await handleDownload(track)
    }
  }

  function handlePlaylistPlay() {
    playPlaylist(playablePlaylist, { startIndex: 0 })
  }

  function handlePlaylistShuffle() {
    playPlaylist(playablePlaylist, { shuffle: 'on' })
  }

  const handleShowInfo = (track: Track) => {
    setInfoTrack(track)
  }

  if (playlistTracks.length === 0) {
    const emptyState = getEmptyStateCopy(
      libraryTracks.length,
      playlistId,
      isCustom,
    )

    return (
      <div className="animate-fade-up flex flex-col items-center justify-center gap-3 px-6 py-20 text-center">
        <div className="flex size-14 items-center justify-center rounded-full border border-border bg-card text-muted-foreground">
          <Music className="size-6" />
        </div>
        <p className="text-base font-medium text-foreground">
          {emptyState.title}
        </p>
        <p className="max-w-xs text-sm text-muted-foreground">
          {emptyState.description}
        </p>
      </div>
    )
  }

  const customPlaylists = data?.custom ?? []

  return (
    <>
      <div className="flex min-h-0 grow flex-col gap-4 pt-4">
        <div className="flex h-fit w-full shrink-0 items-center justify-between gap-4 px-4">
          <div className="grow flex gap-2">
            <Button size="icon" onClick={handlePlaylistPlay}>
              <Play className="size-4" />
            </Button>
            <Button variant="secondary" onClick={handlePlaylistShuffle}>
              <Shuffle className="size-4" />
              Shuffle
            </Button>

            <AnimatePresence initial={false}>
              {selectionMode && (
                <motion.div
                  key="bulk-actions"
                  initial={{ opacity: 0, x: -8 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: -8 }}
                  transition={{ duration: 0.18, ease: 'easeOut' }}
                  className="flex items-center gap-2"
                >
                  <Separator orientation="vertical" className="h-full" />

                  {selectedTrackIds.length > 0 && (
                    <PlaylistBulkActions
                      selectedTrackIds={selectedTrackIds}
                      currentPlaylist={selectedPlaylist}
                      customPlaylists={customPlaylists}
                      likedTrackIds={likedTrackIds}
                      onAddToLiked={handleBulkAddToLiked}
                      onRemoveFromLiked={handleBulkRemoveFromLiked}
                      onAddToPlaylist={handleBulkAddToPlaylist}
                      onRemoveFromPlaylist={handleBulkRemoveFromPlaylist}
                      onDownload={handleBulkDownload}
                    />
                  )}

                  <Button variant="outline" size="icon" onClick={handleExitSelection}>
                    <Undo2 className="size-4" />
                  </Button>
                </motion.div>
              )}
            </AnimatePresence>
          </div>

          <div className="min-w-40 max-w-xl w-full px-2">
            <InputGroup>
              <InputGroupInput
                value={search}
                onChange={e => setSearch(e.target.value)}
                placeholder="Search"
              />
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
        </div>

        <PlaylistTracksTable
          tracks={filteredTracks}
          currentPlaylist={selectedPlaylist}
          customPlaylists={customPlaylists}
          currentTrackId={currentTrackId}
          isPlaying={isPlaying}
          isTrackLiked={trackId => isTrackLiked(data, trackId)}
          selectionMode={selectionMode}
          rowSelection={rowSelection}
          onRowSelectionChange={setRowSelection}
          sorting={sorting}
          onSortingChange={setSorting}
          canReorder={canReorder}
          onReorderTracks={handleReorderTracks}
          onEnterSelection={handleEnterSelection}
          onTrackPlay={handleTrackSelect}
          onToggleLike={handleToggleLike}
          onAddToPlaylist={handleAddToPlaylist}
          onDeleteFromPlaylist={handleDeleteFromPlaylist}
          onDownload={handleDownload}
          onShowInfo={handleShowInfo}
        />
      </div>

      <TrackInfoDialog
        track={infoTrack}
        open={infoTrack !== null}
        onOpenChange={(open) => {
          if (!open) {
            setInfoTrack(null)
          }
        }}
      />
    </>
  )
}
