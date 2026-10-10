import { useMemo } from 'react'
import type { SortingState } from '@tanstack/react-table'
import { useLibraryStore } from '@/stores/library-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { getLikedTrackIdSet, playlistEntries, resolveSelectedPlaylistTracks } from '@/lib/playlists'
import { sortPlaylistEntries } from '@/lib/playlist-track-actions'
import { useFilter } from '@/hooks/utils/use-filter'

/** Derive one membership model for rendering, selection, and playback. */
export function usePlaylistTracks(search: string, sorting: SortingState) {
  const library = useLibraryStore(state => state.library)
  const stats = useListenStatsStore(state => state.statsByTrackId)
  const data = usePlaylistsStore(state => state.data)
  const selectedId = usePlaylistsStore(state => state.selectedPlaylistId)
  const { contains } = useFilter()
  const playlist = useMemo(() => resolveSelectedPlaylistTracks(library, data, selectedId, stats), [library, data, selectedId, stats])
  const entries = useMemo(() => playlistEntries(playlist), [playlist])
  const playableEntries = useMemo(() => sortPlaylistEntries(entries, sorting), [entries, sorting])
  const filteredEntries = useMemo(() => playableEntries.filter(({ track }) => contains(`${track.performer} - ${track.title}`, search)), [playableEntries, contains, search])
  const likedIds = useMemo(() => getLikedTrackIdSet(data), [data])
  return { playlist, entries, filteredEntries, playableEntries, likedIds, custom: data?.custom ?? [], libraryCount: library.length }
}
