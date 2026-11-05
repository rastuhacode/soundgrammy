import { useEffect, useRef, useState } from 'react'
import { api, fileSrc, onArtworkChanged, onCacheChanged } from '@/lib/api'
import { captureSession, isSessionCurrent, useSessionStore } from '@/stores/session-store'

interface ThumbnailState {
  url: string | null
  loaded: boolean
  failed: boolean
  onError: () => void
}

// Cache the resolved cache-file path per track for the session so re-mounting
// virtualized rows doesn't re-invoke the backend.
const pathCache = new Map<string, string | null>()
const pathRevisions = new Map<string, number>()
const requestCache = new Map<string, Promise<string | null>>()
let thumbnailRevision = 0
let cacheGeneration = 0

export function clearThumbnailMemoryCache(): void {
  cacheGeneration++
  pathCache.clear()
  pathRevisions.clear()
  requestCache.clear()
}

export function loadThumbnailPath(
  trackId: number,
  highQuality: boolean,
  revalidate = false,
): Promise<string | null> {
  const cacheKey = `${trackId}:${highQuality ? 'high' : 'standard'}`
  if (!revalidate && !highQuality && pathCache.has(cacheKey)) {
    return Promise.resolve(pathCache.get(cacheKey) ?? null)
  }

  const pending = requestCache.get(cacheKey)
  if (pending) return pending

  const generation = cacheGeneration
  const session = captureSession()
  const pathRevision = pathRevisions.get(cacheKey) ?? 0
  const request = api
    .getTrackThumbnail(trackId, highQuality)
    .then((path) => {
      if (generation !== cacheGeneration || !isSessionCurrent(session)) return null
      if ((pathRevisions.get(cacheKey) ?? 0) !== pathRevision) {
        return pathCache.get(cacheKey) ?? null
      }
      pathCache.set(cacheKey, path)
      return path
    })
    .finally(() => {
      if (requestCache.get(cacheKey) === request) requestCache.delete(cacheKey)
    })
  requestCache.set(cacheKey, request)
  return request
}

function thumbnailUrl(path: string | null, revision = 0): string | null {
  if (!path) return null
  const url = fileSrc(path)
  return revision ? `${url}${url.includes('?') ? '&' : '?'}thumbnailRevision=${revision}` : url
}

function cachedThumbnailState(cacheKey: string, trackId: number, highQuality: boolean) {
  const path = pathCache.get(cacheKey)
    ?? (highQuality ? pathCache.get(`${trackId}:standard`) : null)
  return {
    key: cacheKey,
    url: thumbnailUrl(path ?? null),
    loaded: Boolean(path),
    failed: !path && pathCache.has(cacheKey) && pathCache.get(cacheKey) === null,
  }
}

export function useCachedThumbnail(
  trackId: number,
  options?: { enabled?: boolean, quality?: 'standard' | 'high' },
): ThumbnailState {
  const enabled = options?.enabled ?? true
  const quality = options?.quality ?? 'standard'
  const cacheKey = `${trackId}:${quality}`
  const highQuality = quality === 'high'
  const [revision, setRevision] = useState(0)
  const retries = useRef(0)
  const [state, setState] = useState(() => cachedThumbnailState(cacheKey, trackId, highQuality))

  useEffect(() => {
    retries.current = 0
  }, [cacheKey])

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect -- Thumbnail state mirrors an external cache/backend lookup keyed by trackId. */
    if (!enabled || !trackId) return
    const generation = cacheGeneration
    const session = captureSession()
    let cancelled = false
    let requestVersion = 0
    let pendingImage: HTMLImageElement | undefined
    const stops: Array<() => void> = []
    const current = () => !cancelled && generation === cacheGeneration && isSessionCurrent(session)
    const show = (path: string | null, version: number, preload = false) => {
      if (!current() || version !== requestVersion) return
      const url = thumbnailUrl(path, revision)
      if (preload && !url) return
      const commit = () => {
        if (current() && version === requestVersion) {
          setState(previous => highQuality && !url && previous.key === cacheKey && previous.url
            ? previous
            : { key: cacheKey, url, loaded: Boolean(path), failed: path === null })
        }
      }
      if (preload && url) {
        if (pendingImage) pendingImage.onload = pendingImage.onerror = null
        pendingImage = new Image()
        pendingImage.onload = commit
        pendingImage.onerror = () => {} // Preserve the displayed fallback if the upgrade fails.
        pendingImage.src = url
      }
      else commit()
    }
    const load = async (revalidate = false) => {
      const version = ++requestVersion
      try {
        // A completion event can arrive during the original thumbnail lookup.
        // Let it settle before requesting the newly completed audio's artwork.
        if (revalidate) await requestCache.get(cacheKey)?.catch(() => null)
        if (!current() || version !== requestVersion) return
        const path = await loadThumbnailPath(trackId, highQuality, revalidate || revision > 0)
        show(path, version, highQuality && revalidate)
      }
      catch {
        if (current() && version === requestVersion) {
          setState(previous => previous.key === cacheKey && previous.url
            ? previous
            : { key: cacheKey, url: null, loaded: false, failed: true })
        }
      }
    }
    const subscribe = async () => {
      if (highQuality) {
        // Register before invoking Rust so a fast cached-range probe cannot be missed.
        const subscriptions = await Promise.allSettled([
          onArtworkChanged((artwork) => {
            if (!current() || artwork.trackId !== trackId
              || artwork.tgUserId !== useSessionStore.getState().session?.tgUserId) return
            pathRevisions.set(cacheKey, (pathRevisions.get(cacheKey) ?? 0) + 1)
            pathCache.set(cacheKey, artwork.path)
            show(artwork.path, ++requestVersion, true)
          }),
          onCacheChanged((change) => {
            if (current() && change.cached && change.trackIds.includes(trackId)) void load(true)
          }),
        ])
        for (const subscription of subscriptions) {
          if (subscription.status === 'fulfilled') {
            if (cancelled) subscription.value()
            else stops.push(subscription.value)
          }
        }
      }
      if (current()) void load()
    }
    setState(previous => previous.key === cacheKey
      ? previous
      : cachedThumbnailState(cacheKey, trackId, highQuality))
    void subscribe()
    return () => {
      cancelled = true
      stops.forEach(stop => stop())
      if (pendingImage) pendingImage.onload = pendingImage.onerror = null
    }
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [cacheKey, enabled, highQuality, revision, trackId])

  useEffect(() => {
    if (!enabled || !trackId) return
    const refresh = () => {
      if (document.visibilityState === 'hidden') return
      retries.current = 0
      setRevision(++thumbnailRevision)
    }
    document.addEventListener('visibilitychange', refresh)
    window.addEventListener('pageshow', refresh)
    return () => {
      document.removeEventListener('visibilitychange', refresh)
      window.removeEventListener('pageshow', refresh)
    }
  }, [enabled, trackId])

  const onError = () => {
    if (retries.current < 1) {
      retries.current += 1
      setRevision(++thumbnailRevision)
    }
    else {
      setState({ key: cacheKey, url: null, loaded: false, failed: true })
    }
  }

  const displayed = state.key === cacheKey ? state : cachedThumbnailState(cacheKey, trackId, highQuality)
  return { url: displayed.url, loaded: displayed.loaded, failed: displayed.failed, onError }
}
