import { assertSession, captureSession } from '@/stores/session-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useLibraryStore } from '@/stores/library-store'
import { usePlayerStore } from '@/stores/player-store'
import type { PlaylistId } from '@/lib/playlists'

/** Persist membership first, then align its native playback mirror. */
export async function reorderPlaylistAndQueue(
  id: PlaylistId,
  trackIds: number[],
  expectedTrackIds: number[],
  move: { fromIndex: number, toIndex: number },
) {
  const generation = captureSession()
  await usePlaylistsStore.getState().reorderTracks(id, trackIds, expectedTrackIds)
  assertSession(generation)
  const byId = new Map(useLibraryStore.getState().library.map(track => [track.id, track]))
  const tracks = trackIds.flatMap(id => byId.has(id) ? [byId.get(id)!] : [])
  await usePlayerStore.getState().realignQueueToPlaylist(id, tracks, move)
}
