// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { appLogger } from '@/lib/app-logger'
import { artworkColorRange, useImagePalette } from './use-image-palette'

vi.mock('@/lib/app-logger', () => ({ appLogger: { warning: vi.fn() } }))

interface TestImage {
  crossOrigin: string | null
  onload: (() => void) | null
  onerror: (() => void) | null
  corsAtLoad: string | null
}

let root: Root
let container: HTMLDivElement
let images: TestImage[]
let drawnImage: TestImage
let pixelRead: ReturnType<typeof vi.fn>

function Palette({ url }: { url: string | null }) {
  const { colors, accent } = useImagePalette(url)
  return createElement('output', { 'data-accent': accent ?? undefined }, colors.join(','))
}

async function show(url: string | null) {
  await act(async () => root.render(createElement(Palette, { url })))
}

beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  images = []
  vi.stubGlobal('Image', class implements TestImage {
    crossOrigin = null as string | null
    decoding = ''
    onload = null
    onerror = null
    corsAtLoad = null as string | null
    constructor() { images.push(this) }
    set src(_url: string) { this.corsAtLoad = this.crossOrigin }
  })
  pixelRead = vi.fn(() => {
    // Displaying a cross-origin image works without CORS, but reading the
    // resulting canvas throws. Model that distinction rather than a load error.
    if (drawnImage.corsAtLoad !== 'anonymous') throw new DOMException('Tainted canvas', 'SecurityError')
    return { data: Uint8ClampedArray.from([192, 224, 160, 255, 255, 128, 192, 255, 160, 96, 160, 255]) }
  })
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    drawImage: (image: TestImage) => { drawnImage = image },
    getImageData: pixelRead,
  } as unknown as CanvasRenderingContext2D)
  container = document.createElement('div')
  document.body.append(container)
  root = createRoot(container)
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  vi.clearAllMocks()
})

describe('artwork palette loading', () => {
  it('extracts actual colors from a CORS-enabled Tauri asset instead of the gray fallback', async () => {
    await show('asset://localhost/cache/cover.jpg')
    await act(async () => images[0].onload?.())
    expect(container.textContent).toBe('#c0e0a0,#ff80c0,#a060a0')
    expect(appLogger.warning).not.toHaveBeenCalled()
  })

  it.each([
    { cover: 'navy shadows', shadow: [4, 6, 12], accent: [0, 0, 0], expected: '#04060c' },
    { cover: 'black with sparse blue pixels', shadow: [0, 0, 0], accent: [0, 20, 30], expected: '#000000' },
  ])('preserves the predominant shadow color in $cover', async ({ shadow, accent, expected }) => {
    const pixels = new Uint8ClampedArray(48 * 48 * 4)
    for (let i = 0; i < pixels.length; i += 4) {
      pixels.set([...(i < 16 ? accent : shadow), 255], i)
    }
    pixelRead.mockReturnValue({ data: pixels })
    await show('asset://localhost/cache/shadow.jpg')
    await act(async () => images[0].onload?.())

    const palette = container.textContent!.split(',')
    expect(palette[0]).toBe(expected)
    expect(palette.map(color => artworkColorRange(color).peak !== 'transparent')).toEqual(
      Array(3).fill(expected !== '#000000'),
    )
    if (expected === '#000000') expect(palette).toEqual(Array(3).fill(expected))
    expect(container.querySelector('output')?.hasAttribute('data-accent')).toBe(false)
  })

  it('recovers a recurring blue accent behind dominant grays for use on hits', async () => {
    const pixels = new Uint8ClampedArray(48 * 48 * 4)
    for (let i = 0; i < pixels.length; i += 4) {
      const gray = i < 3_072 ? 224 : i < 6_144 ? 128 : 32
      pixels.set(i < 288 ? [96, 104, 163, 255] : [gray, gray, gray, 255], i)
    }
    pixelRead.mockReturnValue({ data: pixels })
    await show('asset://localhost/cache/blue-detail.jpg')
    await act(async () => images[0].onload?.())

    expect(container.textContent).toBe('#808080,#202020,#e0e0e0')
    const accent = container.querySelector('output')?.getAttribute('data-accent')
    expect(accent).toBe('#6068a3')
    const accentPeak = artworkColorRange(accent!, { recoveredAccent: true }).peak
    const tones = artworkColorRange('#808080', { accentPeak })
    expect(tones.rest).toBe(artworkColorRange('#808080').rest)
    expect(tones.active).toBe('transparent')
    expect(tones.peak).toBe(accentPeak)
  })

  it('does not replace the current artwork colors with a stale image load', async () => {
    await show('asset://localhost/cache/old.jpg')
    await show('asset://localhost/cache/current.jpg')
    await act(async () => images[1].onload?.())
    const current = container.textContent
    await act(async () => images[0].onload?.())
    expect(container.textContent).toBe(current)
    expect(pixelRead).toHaveBeenCalledTimes(1)
  })

  it('reports blocked pixel access rather than silently hiding extraction failures', async () => {
    pixelRead.mockImplementation(() => {
      throw new DOMException('Tainted canvas', 'SecurityError')
    })
    await show('asset://localhost/cache/cover.jpg')
    await act(async () => images[0].onload?.())
    expect(container.textContent).toBe('#141414,#202020,#272727')
    expect(appLogger.warning).toHaveBeenCalledWith(expect.objectContaining({
      title: 'Artwork palette extraction failed',
      context: { errorName: 'SecurityError' },
    }))
  })
})
