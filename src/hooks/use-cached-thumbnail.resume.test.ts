// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { clearThumbnailMemoryCache, useCachedThumbnail } from './use-cached-thumbnail'

const roots: Array<ReturnType<typeof createRoot>> = []

function Thumbnail() {
  const { url, failed, onError } = useCachedThumbnail(42)
  return url && !failed
    ? createElement('img', { src: url, onError, alt: 'Thumbnail' })
    : createElement('span', null, 'No cover')
}

async function mountThumbnail() {
  const container = document.createElement('div')
  document.body.append(container)
  const root = createRoot(container)
  roots.push(root)
  await act(async () => root.render(createElement(Thumbnail)))
  return container
}

afterEach(async () => {
  await act(async () => roots.splice(0).forEach(root => root.unmount()))
  document.body.replaceChildren()
  clearThumbnailMemoryCache()
  vi.restoreAllMocks()
  delete (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
})

describe('thumbnail recovery', () => {
  it('asks Rust again and refreshes the image URL when the app resumes', async () => {
    ;(window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {
      convertFileSrc: (path: string) => `asset://${path}`,
    }
    const backend = vi.spyOn(api, 'getTrackThumbnail').mockResolvedValue('/cache/42.jpg')
    const container = await mountThumbnail()
    const firstUrl = container.querySelector('img')?.src

    await act(async () => window.dispatchEvent(new Event('pageshow')))

    expect(backend).toHaveBeenCalledTimes(2)
    expect(container.querySelector('img')?.src).not.toBe(firstUrl)
  })

  it('retries a failed asset load once and then shows the fallback', async () => {
    ;(window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {
      convertFileSrc: (path: string) => `asset://${path}`,
    }
    const backend = vi.spyOn(api, 'getTrackThumbnail').mockResolvedValue('/cache/42.jpg')
    const container = await mountThumbnail()

    await act(async () => container.querySelector('img')?.dispatchEvent(new Event('error')))
    expect(backend).toHaveBeenCalledTimes(2)
    expect(container.querySelector('img')).not.toBeNull()

    await act(async () => container.querySelector('img')?.dispatchEvent(new Event('error')))
    expect(container.querySelector('img')).toBeNull()
    expect(container.textContent).toBe('No cover')
    expect(backend).toHaveBeenCalledTimes(2)
  })
})
