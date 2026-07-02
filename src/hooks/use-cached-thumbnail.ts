import { useEffect, useState } from 'react'
import { api, fileSrc } from '@/lib/api'

interface ThumbnailState {
  url: string | null
  loaded: boolean
  failed: boolean
}

// Cache the resolved cache-file path per track for the session so re-mounting
// virtualized rows doesn't re-invoke the backend.
const pathCache = new Map<number, string | null>()

export function useCachedThumbnail(
  trackId: number,
  options?: { enabled?: boolean },
): ThumbnailState {
  const enabled = options?.enabled ?? true
  const [state, setState] = useState<ThumbnailState>(() => {
    if (pathCache.has(trackId)) {
      const path = pathCache.get(trackId) ?? null
      return { url: path ? fileSrc(path) : null, loaded: Boolean(path), failed: path === null }
    }
    return { url: null, loaded: false, failed: false }
  })

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect -- Thumbnail state mirrors an external cache/backend lookup keyed by trackId. */
    if (!enabled || !trackId) return

    if (pathCache.has(trackId)) {
      const path = pathCache.get(trackId) ?? null
      setState({
        url: path ? fileSrc(path) : null,
        loaded: Boolean(path),
        failed: path === null,
      })
      return
    }

    let cancelled = false
    api
      .getTrackThumbnail(trackId)
      .then((path) => {
        if (cancelled) return
        pathCache.set(trackId, path)
        setState({
          url: path ? fileSrc(path) : null,
          loaded: Boolean(path),
          failed: path === null,
        })
      })
      .catch(() => {
        if (!cancelled) setState({ url: null, loaded: false, failed: true })
      })

    return () => {
      cancelled = true
    }
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [enabled, trackId])

  return state
}
