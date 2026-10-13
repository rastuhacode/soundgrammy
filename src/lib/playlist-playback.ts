import { assertSession, captureSession } from '@/stores/session-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import type { PlaylistId } from '@/lib/playlists'

/** Rust commits membership and reconciles playback before acknowledging the change. */
export async function reorderPlaylistAndQueue(
  id: PlaylistId,
  trackIds: number[],
  expectedTrackIds: number[],
  move: { fromIndex: number, toIndex: number },
) {
  const generation = captureSession()
  await usePlaylistsStore.getState().reorderTracks(id, trackIds, expectedTrackIds, [move.fromIndex, move.toIndex])
  assertSession(generation)
}
