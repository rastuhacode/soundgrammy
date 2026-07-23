import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  startDownloadProgressListener,
  useCacheStore,
} from './cache-store'
import { onDownloadProgress } from '@/lib/api'

vi.mock('@/lib/api', () => ({
  api: {
    getCacheStatus: vi.fn(async () => [1, 2, 3]),
  },
  onCacheChanged: vi.fn(async () => () => {}),
  onDownloadProgress: vi.fn(async () => () => {}),
}))

describe('useCacheStore', () => {
  beforeEach(() => {
    useCacheStore.setState({
      cachedIds: new Set(),
      busyIds: new Set(),
      progressById: new Map(),
      hydrated: false,
    })
  })

  it('marks tracks cached and uncached', () => {
    const store = useCacheStore.getState()
    store.markCached([1, 2])
    expect(useCacheStore.getState().isCached(1)).toBe(true)
    expect(useCacheStore.getState().isCached(2)).toBe(true)
    expect(useCacheStore.getState().isCached(3)).toBe(false)

    store.markUncached([2])
    expect(useCacheStore.getState().isCached(1)).toBe(true)
    expect(useCacheStore.getState().isCached(2)).toBe(false)
  })

  it('treats empty mark lists as no-ops', () => {
    useCacheStore.getState().markCached([1])
    useCacheStore.getState().markCached([])
    useCacheStore.getState().markUncached([])
    expect([...useCacheStore.getState().cachedIds]).toEqual([1])
  })

  it('clears all cached ids', () => {
    useCacheStore.getState().markCached([1, 2, 3])
    useCacheStore.getState().markBusy([1])
    useCacheStore.getState().setProgress(1, 0.5)
    useCacheStore.getState().clearAll()
    expect(useCacheStore.getState().cachedIds.size).toBe(0)
    expect(useCacheStore.getState().busyIds.size).toBe(0)
    expect(useCacheStore.getState().progressById.size).toBe(0)
  })

  it('reports playlist cached only when every member is cached', () => {
    useCacheStore.getState().markCached([1, 2])
    expect(useCacheStore.getState().isPlaylistCached([])).toBe(false)
    expect(useCacheStore.getState().isPlaylistCached([1, 2])).toBe(true)
    expect(useCacheStore.getState().isPlaylistCached([1, 2, 3])).toBe(false)
  })

  it('hydrates from getCacheStatus', async () => {
    await useCacheStore.getState().hydrate()
    expect(useCacheStore.getState().hydrated).toBe(true)
    expect([...useCacheStore.getState().cachedIds].sort()).toEqual([1, 2, 3])
  })

  it('tracks busy ids and clears progress when clearing busy', () => {
    const store = useCacheStore.getState()
    store.markBusy([1, 2])
    store.setProgress(1, 0.4)
    expect(store.isBusy(1)).toBe(true)
    expect(store.isBusy(2)).toBe(true)
    expect(useCacheStore.getState().progressById.get(1)).toBe(0.4)

    store.clearBusy([1])
    expect(useCacheStore.getState().isBusy(1)).toBe(false)
    expect(useCacheStore.getState().isBusy(2)).toBe(true)
    expect(useCacheStore.getState().progressById.has(1)).toBe(false)
  })

  it('clamps progress into 0..1', () => {
    useCacheStore.getState().setProgress(1, 1.5)
    expect(useCacheStore.getState().progressById.get(1)).toBe(1)
    useCacheStore.getState().setProgress(1, -0.2)
    expect(useCacheStore.getState().progressById.get(1)).toBe(0)
  })

  it('updates progress from download:progress events', async () => {
    let listener:
      | ((p: {
        trackId: number
        received: number
        total: number
        ranges: Array<{ start: number, end: number }>
        complete: boolean
      }) => void)
      | undefined

    const onProgress = onDownloadProgress as unknown as ReturnType<typeof vi.fn>
    onProgress.mockImplementation(async (cb: typeof listener) => {
      listener = cb
      return () => {}
    })

    await startDownloadProgressListener()
    expect(listener).toBeDefined()

    listener!({
      trackId: 7,
      received: 25,
      total: 100,
      ranges: [],
      complete: false,
    })
    expect(useCacheStore.getState().progressById.get(7)).toBe(0.25)

    listener!({
      trackId: 7,
      received: 100,
      total: 100,
      ranges: [],
      complete: true,
    })
    expect(useCacheStore.getState().progressById.has(7)).toBe(false)
  })
})
