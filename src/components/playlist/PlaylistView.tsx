import type { Track } from '@/lib/db'
import { useLibraryStore } from '@/stores/library-store'
import { usePlayerStore } from '@/stores/player-store'
import {
  isTrackLiked,
  LIKED_PLAYLIST_ID,
  resolveSelectedPlaylistTracks,
  usePlaylistsStore,
} from '@/stores/playlists-store'
import type {
  CustomPlaylistId,
  ResolvedSelectedPlaylist,
} from '@/stores/playlists-store'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Music, Play, Shuffle } from 'lucide-react'
import { useMemo, useRef, useState } from 'react'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { Separator } from '@/components/ui/separator'
import { api } from '@/lib/api'

import { TRACK_ROW_HEIGHT, PlaylistTrackRow } from './PlaylistTrackRow'
import { TrackInfoDialog } from './TrackInfoDialog'
import { Button } from '@/components/ui/button'

function getEmptyStateCopy(
  libraryTrackCount: number,
  playlistId: ResolvedSelectedPlaylist['id'],
  isCustom: boolean,
): { title: string, description: string } {
  if (libraryTrackCount === 0) {
    return {
      title: 'No playlistTracks yet',
      description:
        'Pin music to your Telegram profile and it will tune in here automatically.',
    }
  }

  if (playlistId === LIKED_PLAYLIST_ID) {
    return {
      title: 'No liked playlistTracks yet',
      description: 'Tap the heart on any track to save it here.',
    }
  }

  if (isCustom) {
    return {
      title: 'This playlist is empty',
      description: 'Add playlistTracks from your library using the list button.',
    }
  }

  return {
    title: 'No playlistTracks yet',
    description:
      'Pin music to your Telegram profile and it will tune in here automatically.',
  }
}

export function PlaylistView() {
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
  const scrollRef = useRef<HTMLDivElement>(null)
  const [infoTrack, setInfoTrack] = useState<Track | null>(null)

  const selectedPlaylist = useMemo(
    () => resolveSelectedPlaylistTracks(libraryTracks, data, selectedPlaylistId),
    [libraryTracks, data, selectedPlaylistId],
  )
  const {
    tracks: playlistTracks,
    isCustom,
    name: playlistName,
    id: playlistId,
  } = selectedPlaylist

  // TanStack Virtual intentionally returns live functions
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: playlistTracks.length,
    gap: 8,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => TRACK_ROW_HEIGHT,
    overscan: 8,
  })

  const handleTrackSelect = (track: Track, startIndex: number) => {
    playPlaylist(selectedPlaylist, { start: track, startIndex })
  }

  const handleToggleLike = async (trackId: number) => {
    if (!data) return
    try {
      const trackIds = await api.toggleLike(trackId)
      setData({ ...data, liked: { ...data.liked, trackIds } })
    }
    catch {
      // keep UI unchanged on failure
    }
  }

  const handleAddToPlaylist = async (targetId: number, trackId: number) => {
    if (!data) return
    try {
      await api.addTrackToPlaylist(targetId, trackId)
      setData({
        ...data,
        custom: data.custom.map(playlist =>
          playlist.id === targetId
            ? {
                ...playlist,
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

  const handleDeleteFromPlaylist = async (
    targetId: CustomPlaylistId,
    trackId: number,
  ) => {
    if (!data) return
    try {
      await api.removeTrackFromPlaylist(targetId, trackId)
      setData({
        ...data,
        custom: data.custom.map(playlist =>
          playlist.id === targetId
            ? {
                ...playlist,
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

  const handleDownload = async (track: Track) => {
    try {
      const path = await api.getTrackSource(track.id)
      await revealItemInDir(path)
    }
    catch {
      // ignore — file may still be downloading
    }
  }

  function handlePlaylistPlay() {
    playPlaylist(selectedPlaylist, { startIndex: 0 })
  }

  function handlePlaylistShuffle() {
    playPlaylist(selectedPlaylist, { shuffle: 'on' })
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
      <div className="flex min-h-0 grow flex-col pt-4">
        <div className="h-10 px-4 shrink-0 flex items-center gap-4">
          <h2 className="text-lg font-semibold">{playlistName}</h2>
          <Button onClick={handlePlaylistPlay}>
            <Play className="size-4" />
            Play
          </Button>
          <Button variant="secondary" onClick={handlePlaylistShuffle}>
            <Shuffle className="size-4" />
            Shuffle
          </Button>
        </div>

        <Separator className="mt-4" />

        <div ref={scrollRef} className="min-h-0 grow overflow-y-auto p-4">
          <ul
            className="relative w-full list-none"
            style={{ height: `${virtualizer.getTotalSize()}px` }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const track = playlistTracks[virtualRow.index]
              if (!track) return null

              return (
                <PlaylistTrackRow
                  key={track.id}
                  className="absolute left-0 top-0 w-full"
                  style={{ height: `${virtualRow.size}px`, transform: `translateY(${virtualRow.start}px)` }}
                  currentPlaylist={selectedPlaylist}
                  track={track}
                  isActive={currentTrackId === track.id}
                  isPlaying={isPlaying}
                  isLiked={isTrackLiked(data, track.id)}
                  customPlaylists={customPlaylists}
                  onTrackSelect={track =>
                    handleTrackSelect(track, virtualRow.index)}
                  onToggleLike={handleToggleLike}
                  onAddToPlaylist={handleAddToPlaylist}
                  onDeleteFromPlaylist={handleDeleteFromPlaylist}
                  onDownload={handleDownload}
                  onShowInfo={handleShowInfo}
                />
              )
            })}
          </ul>
        </div>
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
