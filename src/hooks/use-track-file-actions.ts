import type { Track } from '@/types'
import { api } from '@/lib/api'
import { errorMessage } from '@/lib/errors'
import { exportTrackAndReveal } from '@/lib/export-track'
import { useCacheStore, withBusyTracks } from '@/stores/cache-store'
import { captureSession, isSessionCurrent } from '@/stores/session-store'

/** Track file operations share busy ownership, cleanup and session validity. */
export function useTrackFileActions(onError: (message: string) => void) {
  const run = async (trackIds: number[], operation: (ids: number[]) => Promise<unknown>) => {
    const generation = captureSession()
    const ids = [...new Set(trackIds)].filter(id => !useCacheStore.getState().isBusy(id))
    if (!ids.length) return
    try {
      await withBusyTracks(ids, () => operation(ids))
    }
    catch (error) {
      if (isSessionCurrent(generation)) onError(errorMessage(error))
    }
  }
  const cache = (trackIds: number[]) => run(trackIds, async (ids) => {
    const generation = captureSession()
    const cached = await api.cacheTracks(ids)
    if (isSessionCurrent(generation)) useCacheStore.getState().markCached(cached)
  })
  const remove = async (track: Track) => {
    const generation = captureSession()
    try {
      await api.removeTrackFromCache(track.id)
      if (isSessionCurrent(generation)) useCacheStore.getState().markUncached([track.id])
    }
    catch (error) {
      if (isSessionCurrent(generation)) onError(errorMessage(error))
    }
  }
  return {
    handleCache: (track: Track) => run([track.id], async () => {
      const generation = captureSession()
      await api.cacheTrack(track.id)
      if (isSessionCurrent(generation)) useCacheStore.getState().markCached([track.id])
    }),
    handleBulkCache: cache,
    handleDownload: (track: Track) => run([track.id], () => exportTrackAndReveal(track.id)),
    handleBulkDownload: (trackIds: number[]) => run(trackIds, ids => api.exportTracks(ids)),
    handleRemoveFromCache: remove,
  }
}
