import { afterEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import {
  clearThumbnailMemoryCache,
  loadThumbnailPath,
} from './use-cached-thumbnail'

afterEach(() => {
  clearThumbnailMemoryCache()
  vi.restoreAllMocks()
})

describe('loadThumbnailPath', () => {
  it('shares one backend request between concurrent consumers', async () => {
    let resolveRequest!: (path: string | null) => void
    const backend = vi.spyOn(api, 'getTrackThumbnail').mockReturnValue(
      new Promise(resolve => (resolveRequest = resolve)),
    )

    const first = loadThumbnailPath(42, false)
    const second = loadThumbnailPath(42, false)

    expect(backend).toHaveBeenCalledTimes(1)
    expect(first).toBe(second)

    resolveRequest('/cache/42.jpg')
    await expect(first).resolves.toBe('/cache/42.jpg')
    await expect(second).resolves.toBe('/cache/42.jpg')
  })

  it('reuses completed standard thumbnails without another command', async () => {
    const backend = vi
      .spyOn(api, 'getTrackThumbnail')
      .mockResolvedValue('/cache/42.jpg')

    await expect(loadThumbnailPath(42, false)).resolves.toBe('/cache/42.jpg')
    await expect(loadThumbnailPath(42, false)).resolves.toBe('/cache/42.jpg')
    expect(backend).toHaveBeenCalledTimes(1)
  })

  it('keeps standard and high-quality work independent', async () => {
    const backend = vi
      .spyOn(api, 'getTrackThumbnail')
      .mockImplementation(async (_trackId, highQuality) => (
        highQuality ? '/cache/42.full.jpg' : '/cache/42.jpg'
      ))

    await Promise.all([
      loadThumbnailPath(42, false),
      loadThumbnailPath(42, true),
    ])

    expect(backend).toHaveBeenCalledTimes(2)
    expect(backend).toHaveBeenCalledWith(42, false)
    expect(backend).toHaveBeenCalledWith(42, true)
  })

  it('allows a retry after a failed backend request', async () => {
    const backend = vi
      .spyOn(api, 'getTrackThumbnail')
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce('/cache/42.jpg')

    await expect(loadThumbnailPath(42, false)).rejects.toThrow('offline')
    await expect(loadThumbnailPath(42, false)).resolves.toBe('/cache/42.jpg')
    expect(backend).toHaveBeenCalledTimes(2)
  })

  it('caches a confirmed missing standard thumbnail', async () => {
    const backend = vi.spyOn(api, 'getTrackThumbnail').mockResolvedValue(null)

    await expect(loadThumbnailPath(42, false)).resolves.toBeNull()
    await expect(loadThumbnailPath(42, false)).resolves.toBeNull()
    expect(backend).toHaveBeenCalledTimes(1)
  })

  it('rechecks completed high-quality artwork so embedded art can upgrade it', async () => {
    const backend = vi
      .spyOn(api, 'getTrackThumbnail')
      .mockResolvedValue('/cache/42.full.jpg')

    await loadThumbnailPath(42, true)
    await loadThumbnailPath(42, true)
    expect(backend).toHaveBeenCalledTimes(2)
  })
})
