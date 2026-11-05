// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import * as backend from '@/lib/api'
import type { ArtworkChanged, CacheChanged } from '@/types'
import { useSessionStore } from '@/stores/session-store'
import { clearThumbnailMemoryCache, loadThumbnailPath, useCachedThumbnail } from './use-cached-thumbnail'

let artwork: (value: ArtworkChanged) => void
let cache: (value: CacheChanged) => void
const stops = [vi.fn(), vi.fn()]
const images: Array<{ onload: (() => void) | null, onerror: (() => void) | null, src: string }> = []
let root: ReturnType<typeof createRoot> | undefined

function Cover({ trackId }: { trackId: number }) {
  const { url } = useCachedThumbnail(trackId, { quality: 'high' })
  return createElement('img', { src: url ?? undefined, alt: 'Cover' })
}

async function mount(trackId = 42) {
  const container = document.createElement('div')
  document.body.append(container)
  root = createRoot(container)
  await act(async () => root?.render(createElement(Cover, { trackId })))
  return container
}

beforeEach(() => {
  useSessionStore.getState().setSession({ tgUserId: 7, firstName: 'Test', lastName: null, username: null })
  vi.spyOn(backend, 'onArtworkChanged').mockImplementation(async (callback) => {
    artwork = callback
    return stops[0]
  })
  vi.spyOn(backend, 'onCacheChanged').mockImplementation(async (callback) => {
    cache = callback
    return stops[1]
  })
  vi.stubGlobal('Image', class {
    onload = null
    onerror = null
    src = ''
    constructor() { images.push(this) }
  })
  ;(window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {
    convertFileSrc: (path: string) => `asset://${path}`,
  }
})

afterEach(async () => {
  await act(async () => root?.unmount())
  root = undefined
  document.body.replaceChildren()
  images.length = 0
  clearThumbnailMemoryCache()
  useSessionStore.getState().clearSession()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  stops.forEach(stop => stop.mockClear())
  delete (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
})

describe('high-quality artwork upgrades', () => {
  it('shows an existing standard thumbnail immediately and preserves it if lookup fails', async () => {
    const lookup = vi.spyOn(backend.api, 'getTrackThumbnail').mockResolvedValue('/cache/42.jpg')
    await loadThumbnailPath(42, false)
    lookup.mockRejectedValue(new Error('offline'))
    const container = await mount()
    expect(container.querySelector('img')?.src).toBe('asset:///cache/42.jpg')
  })

  it('keeps the Telegram image visible until the embedded image loads', async () => {
    vi.spyOn(backend.api, 'getTrackThumbnail').mockResolvedValue('/cache/42.full.jpg')
    const container = await mount()
    await act(async () => artwork({ trackId: 42, tgUserId: 7, path: '/cache/42.embed.jpg' }))
    expect(container.querySelector('img')?.src).toBe('asset:///cache/42.full.jpg')
    await act(async () => images.at(-1)?.onload?.())
    expect(container.querySelector('img')?.src).toBe('asset:///cache/42.embed.jpg')
  })

  it('does not let a slow original lookup overwrite a completed upgrade', async () => {
    let resolve!: (path: string | null) => void
    vi.spyOn(backend.api, 'getTrackThumbnail').mockReturnValue(new Promise(done => (resolve = done)))
    const container = await mount()
    await act(async () => artwork({ trackId: 42, tgUserId: 7, path: '/cache/42.embed.jpg' }))
    await act(async () => images.at(-1)?.onload?.())
    await act(async () => resolve('/cache/42.full.jpg'))
    expect(container.querySelector('img')?.src).toBe('asset:///cache/42.embed.jpg')
  })

  it('keeps the fallback if the upgraded image cannot load', async () => {
    vi.spyOn(backend.api, 'getTrackThumbnail').mockResolvedValue('/cache/42.full.jpg')
    const container = await mount()
    await act(async () => artwork({ trackId: 42, tgUserId: 7, path: '/cache/42.embed.jpg' }))
    await act(async () => images.at(-1)?.onerror?.())
    expect(container.querySelector('img')?.src).toBe('asset:///cache/42.full.jpg')
  })

  it('revalidates artwork after audio completes while fullscreen remains mounted', async () => {
    const lookup = vi.spyOn(backend.api, 'getTrackThumbnail')
      .mockResolvedValueOnce('/cache/42.full.jpg')
      .mockResolvedValue('/cache/42.embed.jpg')
    const container = await mount()
    await act(async () => cache({ trackIds: [42], cached: true, cleared: false }))
    expect(lookup).toHaveBeenCalledTimes(2)
    await act(async () => images.at(-1)?.onload?.())
    expect(container.querySelector('img')?.src).toBe('asset:///cache/42.embed.jpg')
  })

  it('revalidates again when completion races the original pending lookup', async () => {
    let resolve!: (path: string | null) => void
    const lookup = vi.spyOn(backend.api, 'getTrackThumbnail')
      .mockReturnValueOnce(new Promise(done => (resolve = done)))
      .mockResolvedValue('/cache/42.embed.jpg')
    const container = await mount()
    await act(async () => cache({ trackIds: [42], cached: true, cleared: false }))
    await act(async () => resolve('/cache/42.full.jpg'))
    expect(lookup).toHaveBeenCalledTimes(2)
    await act(async () => images.at(-1)?.onload?.())
    expect(container.querySelector('img')?.src).toBe('asset:///cache/42.embed.jpg')
  })

  it('ignores other tracks, other accounts, and events after logout', async () => {
    vi.spyOn(backend.api, 'getTrackThumbnail').mockResolvedValue('/cache/42.full.jpg')
    const container = await mount()
    await act(async () => {
      artwork({ trackId: 43, tgUserId: 7, path: '/cache/43.embed.jpg' })
      artwork({ trackId: 42, tgUserId: 8, path: '/cache/other.embed.jpg' })
      useSessionStore.getState().clearSession()
      artwork({ trackId: 42, tgUserId: 7, path: '/cache/42.embed.jpg' })
    })
    expect(images).toHaveLength(0)
    expect(container.querySelector('img')?.src).toBe('asset:///cache/42.full.jpg')
  })

  it('discards pending old-track and old-session results and cleans up listeners', async () => {
    let resolve!: (path: string | null) => void
    vi.spyOn(backend.api, 'getTrackThumbnail')
      .mockReturnValueOnce(new Promise(done => (resolve = done)))
      .mockResolvedValue('/cache/43.full.jpg')
    const container = await mount()
    await act(async () => root?.render(createElement(Cover, { trackId: 43 })))
    await act(async () => resolve('/cache/42.full.jpg'))
    expect(container.querySelector('img')?.src).toBe('asset:///cache/43.full.jpg')
    expect(stops[0]).toHaveBeenCalledTimes(1)
    expect(stops[1]).toHaveBeenCalledTimes(1)

    let resolveOldSession!: (path: string | null) => void
    vi.spyOn(backend.api, 'getTrackThumbnail').mockReturnValue(new Promise(done => (resolveOldSession = done)))
    const pending = loadThumbnailPath(99, true)
    useSessionStore.getState().clearSession()
    resolveOldSession('/cache/99.embed.jpg')
    await expect(pending).resolves.toBeNull()
  })
})
