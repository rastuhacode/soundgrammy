import { captureSession, isSessionCurrent } from '@/stores/session-store'
import { usePlayerStore } from '@/stores/player-store'
import { useLibraryStore } from '@/stores/library-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { useCacheStore } from '@/stores/cache-store'

/** Each domain commits independently; optional features cannot block the library. */
export async function hydrateLibrary(firstLoad = false): Promise<void> {
  const generation = captureSession()
  await Promise.allSettled([
    useLibraryStore.getState().refresh().then((tracks) => {
      if (tracks && isSessionCurrent(generation) && useLibraryStore.getState().library === tracks) {
        usePlayerStore.getState().refreshQueueTracks(tracks)
      }
    }),
    usePlaylistsStore.getState().refresh(firstLoad),
    useListenStatsStore.getState().refresh(),
    useCacheStore.getState().hydrate(),
  ])
}
