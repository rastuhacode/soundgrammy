import { useEffect, useState } from 'react'
import { api, fileSrc } from '@/lib/api'

interface ThumbnailState {
  url: string | null
  loaded: boolean
  failed: boolean
}

// Cache the resolved cache-file path per track for the session so re-mounting
// virtualized rows doesn't re-invoke the backend.
const pathCache = new Map<string, string | null>()

export function useCachedThumbnail(
  trackId: number,
  options?: { enabled?: boolean, quality?: 'standard' | 'high' },
): ThumbnailState {
  const enabled = options?.enabled ?? true
  const quality = options?.quality ?? 'standard'
  const cacheKey = `${trackId}:${quality}`
  const [state, setState] = useState<ThumbnailState>(() => {
    if (pathCache.has(cacheKey)) {
      const path = pathCache.get(cacheKey) ?? null
      return { url: path ? fileSrc(path) : null, loaded: Boolean(path), failed: path === null }
    }
    return { url: null, loaded: false, failed: false }
  })

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect -- Thumbnail state mirrors an external cache/backend lookup keyed by trackId. */
    if (!enabled || !trackId) return

    if (pathCache.has(cacheKey)) {
      const path = pathCache.get(cacheKey) ?? null
      setState({
        url: path ? fileSrc(path) : null,
        loaded: Boolean(path),
        failed: path === null,
      })
      return
    }

    let cancelled = false
    setState({ url: null, loaded: false, failed: false })
    api
      .getTrackThumbnail(trackId, quality === 'high')
      .then((path) => {
        if (cancelled) return
        pathCache.set(cacheKey, path)
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
  }, [cacheKey, enabled, quality, trackId])

  return state
}
