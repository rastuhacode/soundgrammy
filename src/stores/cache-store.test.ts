import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useCacheStore } from './cache-store'

vi.mock('@/lib/api', () => ({
  api: {
    getCacheStatus: vi.fn(async () => [1, 2, 3]),
  },
  onCacheChanged: vi.fn(async () => () => {}),
}))

describe('useCacheStore', () => {
  beforeEach(() => {
    useCacheStore.setState({
      cachedIds: new Set(),
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
    useCacheStore.getState().clearAll()
    expect(useCacheStore.getState().cachedIds.size).toBe(0)
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
})
