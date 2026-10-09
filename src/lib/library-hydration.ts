import { useLibraryStore } from '@/stores/library-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { useCacheStore } from '@/stores/cache-store'

/** Each domain commits independently; optional features cannot block the library. */
export async function hydrateLibrary(firstLoad = false): Promise<void> {
  await Promise.allSettled([
    useLibraryStore.getState().refresh(),
    usePlaylistsStore.getState().refresh(firstLoad),
    useListenStatsStore.getState().refresh(),
    useCacheStore.getState().hydrate(),
  ])
}
