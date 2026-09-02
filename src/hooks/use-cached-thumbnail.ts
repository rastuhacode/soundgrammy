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
const requestCache = new Map<string, Promise<string | null>>()

export function clearThumbnailMemoryCache(): void {
  pathCache.clear()
  requestCache.clear()
}

export function loadThumbnailPath(
  trackId: number,
  highQuality: boolean,
): Promise<string | null> {
  const cacheKey = `${trackId}:${highQuality ? 'high' : 'standard'}`
  if (!highQuality && pathCache.has(cacheKey)) {
    return Promise.resolve(pathCache.get(cacheKey) ?? null)
  }

  const pending = requestCache.get(cacheKey)
  if (pending) return pending

  const request = api
    .getTrackThumbnail(trackId, highQuality)
    .then((path) => {
      pathCache.set(cacheKey, path)
      return path
    })
    .finally(() => {
      requestCache.delete(cacheKey)
    })
  requestCache.set(cacheKey, request)
  return request
}

function thumbnailUrl(path: string | null): string | null {
  return path ? fileSrc(path) : null
}

export function useCachedThumbnail(
  trackId: number,
  options?: { enabled?: boolean, quality?: 'standard' | 'high' },
): ThumbnailState {
  const enabled = options?.enabled ?? true
  const quality = options?.quality ?? 'standard'
  const cacheKey = `${trackId}:${quality}`
  const highQuality = quality === 'high'
  const [state, setState] = useState<ThumbnailState>(() => {
    if (pathCache.has(cacheKey)) {
      const path = pathCache.get(cacheKey) ?? null
      return {
        url: thumbnailUrl(path),
        loaded: Boolean(path),
        failed: path === null,
      }
    }
    return { url: null, loaded: false, failed: false }
  })

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect -- Thumbnail state mirrors an external cache/backend lookup keyed by trackId. */
    if (!enabled || !trackId) return

    // High-quality thumbs can upgrade from remote → embedded once audio is
    // cached, so always re-ask the backend. Standard list thumbs stay session-cached.
    if (!highQuality && pathCache.has(cacheKey)) {
      const path = pathCache.get(cacheKey) ?? null
      setState({
        url: thumbnailUrl(path),
        loaded: Boolean(path),
        failed: path === null,
      })
      return
    }

    let cancelled = false
    if (!pathCache.has(cacheKey)) {
      setState({ url: null, loaded: false, failed: false })
    }

    loadThumbnailPath(trackId, highQuality)
      .then((path) => {
        if (cancelled) return
        setState({
          url: thumbnailUrl(path),
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
  }, [cacheKey, enabled, highQuality, trackId])

  return state
}
