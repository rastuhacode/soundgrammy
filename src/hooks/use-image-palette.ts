import { useEffect, useState } from 'react'
import { appLogger } from '@/lib/app-logger'

const FALLBACK_PALETTE: ArtworkPalette = { colors: ['#141414', '#202020', '#272727'], accent: null }
const SAMPLE_SIZE = 48
const BUCKET_SIZE = 32
const MIN_COLOR_DISTANCE = 72
const MIN_TINT_SAMPLES = Math.ceil(SAMPLE_SIZE ** 2 * 0.02)

type RgbColor = [number, number, number]

interface ArtworkPalette {
  colors: string[]
  accent: string | null
}

interface ColorBucket {
  color: RgbColor
  count: number
  shadowSum?: RgbColor
  chromaSum: number
}

interface ArtworkColorRange {
  rest: string
  active: string
  peak: string
}

interface ArtworkColorOptions {
  accentPeak?: string
  recoveredAccent?: boolean
}

function colorHue([red, green, blue]: RgbColor): number {
  const max = Math.max(red, green, blue)
  const delta = max - Math.min(red, green, blue)
  if (delta === 0) return 0
  const sector = max === red ? (green - blue) / delta : max === green ? (blue - red) / delta + 2 : (red - green) / delta + 4
  return (sector * 60 + 360) % 360
}

/** Keep each hue at its ambient lightness and add chroma on hits. */
export function artworkColorRange(hex: string, { accentPeak, recoveredAccent }: ArtworkColorOptions = {}): ArtworkColorRange {
  const rgb = [1, 3, 5].map(index => Number.parseInt(hex.slice(index, index + 2), 16) / 255) as RgbColor
  const max = Math.max(...rgb)
  const min = Math.min(...rgb)
  const lightness = (max + min) / 2
  const delta = max - min
  const saturation = delta === 0 ? 0 : delta / (1 - Math.abs(2 * lightness - 1))
  const hue = colorHue(rgb)
  const level = Math.min(0.36, lightness * 0.65)
  const tone = (value: number, lightness = level) => `hsl(${hue.toFixed(1)} ${(value * 100).toFixed(1)}% ${(lightness * 100).toFixed(1)}%)`
  const rest = saturation * 0.45
  if (saturation < 0.05) {
    return { rest: tone(rest), active: 'transparent', peak: accentPeak ?? 'transparent' }
  }

  // HSL at 50% lightness encodes chroma directly for the color-blend layers.
  // Their blend uses the backdrop's luminosity, so even a dark tint can send
  // more chroma than its ambient swatch can hold without lifting the backdrop.
  const chromaRange = 2 * level
  const restChroma = rest * chromaRange
  const targetChromaIncrease = 0.10 * Math.min(1, saturation / 0.25)
  const basePeak = Math.max(Math.min(1, saturation * 1.35 + 0.25) * chromaRange, restChroma + targetChromaIncrease)
  // Recovered accents only appear in the translucent peak layer. Give muted
  // ones more chroma there, tapering the boost out for already vivid accents.
  const peak = basePeak + (recoveredAccent ? Math.max(0, 0.40 - basePeak) * 0.75 : 0)
  const active = Math.max(Math.min(1, saturation * 1.05 + 0.12) * chromaRange, restChroma + (basePeak - restChroma) * 0.55)
  return {
    rest: tone(rest),
    active: tone(active, 0.5),
    peak: tone(peak, 0.5),
  }
}

function toHex(value: number): string {
  return Math.max(0, Math.min(255, value)).toString(16).padStart(2, '0')
}

function rgbToHex(color: RgbColor): string {
  return `#${color.map(toHex).join('')}`
}

function colorDistance(a: RgbColor, b: RgbColor): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2])
}

function mixColors(a: RgbColor, b: RgbColor): RgbColor {
  return a.map((value, index) => Math.round((value + b[index]) / 2)) as RgbColor
}

function shiftTone(color: RgbColor, amount: number): RgbColor {
  const target = amount > 0 ? 255 : 0
  return color.map(value => Math.round(value + (target - value) * Math.abs(amount))) as RgbColor
}

export function completeArtworkPalette(colors: RgbColor[]): RgbColor[] {
  if (colors.length === 0) return []
  if (colors.length >= 3) return colors.slice(0, 3)
  if (colors.length === 2) return [colors[0], colors[1], mixColors(colors[0], colors[1])]

  const color = colors[0]
  if (Math.max(...color) < BUCKET_SIZE * 1.5) {
    return [color, shiftTone(color, -0.14), shiftTone(color, -0.25)]
  }
  const brightness = (color[0] + color[1] + color[2]) / 3
  return brightness < 128
    ? [color, shiftTone(color, 0.18), shiftTone(color, -0.25)]
    : [color, shiftTone(color, -0.18), shiftTone(color, 0.12)]
}

function shadowTint({ count, shadowSum, chromaSum }: ColorBucket): RgbColor | undefined {
  if (!shadowSum) return
  const average = shadowSum.map(value => Math.round(value / count)) as RgbColor
  const max = Math.max(...average)
  const min = Math.min(...average)
  const chroma = max - min
  // A visible average tint must survive averaging most of the original chroma.
  // This filters out weak and mixed-color noise around neutral shadows.
  if (chroma >= Math.max(3, (max + min) * 0.08, chromaSum / count * 0.65)) return average
}

/** Recover a repeated accent even if larger gray areas take up the ambient palette. */
function findArtworkAccent(pixels: Uint8ClampedArray, colors: RgbColor[]): string | null {
  const visibleHues = colors
    .filter(color => Math.max(...color) - Math.min(...color) >= 16)
    .map(colorHue)
  if (visibleHues.length >= 2) return null

  const groups = new Map<number, { sum: RgbColor, count: number, chroma: number }>()
  for (let index = 0; index < pixels.length; index += 4) {
    if (pixels[index + 3] < 180) continue
    const color: RgbColor = [pixels[index], pixels[index + 1], pixels[index + 2]]
    const chroma = Math.max(...color) - Math.min(...color)
    if (chroma < 12) continue
    const hue = Math.round(colorHue(color) / 30) % 12 * 30
    let group = groups.get(hue)
    if (!group) {
      group = { sum: [0, 0, 0], count: 0, chroma: 0 }
      groups.set(hue, group)
    }
    for (let channel = 0; channel < 3; channel++) group.sum[channel] += color[channel]
    group.count++
    group.chroma += chroma
  }
  const candidates = [...groups.entries()]
    .filter(([, group]) => group.count >= MIN_TINT_SAMPLES)
    .sort((a, b) => b[1].chroma - a[1].chroma)
  const distinct = candidates.find(([hue]) => visibleHues.every((other) => {
    const distance = Math.abs(hue - other)
    return Math.min(distance, 360 - distance) >= 30
  }))
  const candidate = distinct ?? candidates[0]
  if (!candidate) return null
  const { sum, count } = candidate[1]
  return rgbToHex(sum.map(value => Math.round(value / count)) as RgbColor)
}

function extractPalette(image: HTMLImageElement): ArtworkPalette {
  const canvas = document.createElement('canvas')
  canvas.width = SAMPLE_SIZE
  canvas.height = SAMPLE_SIZE
  const context = canvas.getContext('2d', { willReadFrequently: true })
  if (!context) return FALLBACK_PALETTE

  context.drawImage(image, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE)
  const pixels = context.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE).data
  const buckets = new Map<string, ColorBucket>()

  for (let index = 0; index < pixels.length; index += 4) {
    if (pixels[index + 3] < 180) continue
    const red = pixels[index]
    const green = pixels[index + 1]
    const blue = pixels[index + 2]
    const color = [red, green, blue].map(value => Math.min(255, Math.round(value / BUCKET_SIZE) * BUCKET_SIZE)) as RgbColor
    const key = color.join(',')
    let bucket = buckets.get(key)
    if (!bucket) {
      bucket = { color, count: 0, chromaSum: 0 }
      buckets.set(key, bucket)
    }
    bucket.count += 1
    if (Math.max(...color) <= BUCKET_SIZE) {
      const sum = bucket.shadowSum ??= [0, 0, 0]
      sum[0] += red
      sum[1] += green
      sum[2] += blue
      bucket.chromaSum += Math.max(red, green, blue) - Math.min(red, green, blue)
    }
  }

  const candidates = [...buckets.values()].sort((a, b) => b.count - a.count)
  const preferredColors = candidates.flatMap((bucket) => {
    if (bucket.shadowSum && bucket.count < MIN_TINT_SAMPLES) return []
    const tint = shadowTint(bucket)
    if (tint) return [tint]
    const brightness = (bucket.color[0] + bucket.color[1] + bucket.color[2]) / 3
    return brightness >= 12 && brightness <= 244 ? [bucket.color] : []
  })
  const selected: RgbColor[] = []
  for (const candidate of preferredColors.length > 0 ? preferredColors : candidates.map(bucket => bucket.color)) {
    if (selected.every(color => colorDistance(color, candidate) >= MIN_COLOR_DISTANCE)) {
      selected.push(candidate)
    }
    if (selected.length === 3) break
  }

  const palette = completeArtworkPalette(selected)
  if (palette.length === 0) return FALLBACK_PALETTE

  return { colors: palette.map(rgbToHex), accent: findArtworkAccent(pixels, selected) }
}

export function useImagePalette(url: string | null): ArtworkPalette {
  const [result, setResult] = useState<{
    url: string
    palette: ArtworkPalette
  } | null>(null)

  useEffect(() => {
    if (!url) return

    let cancelled = false
    const image = new Image()
    // Tauri artwork is served from the asset protocol, outside the WebView's
    // origin. Opt into its CORS response before loading so canvas pixels remain
    // readable; a normal <img> can display it even when readback is forbidden.
    image.crossOrigin = 'anonymous'
    image.decoding = 'async'
    image.onload = () => {
      if (cancelled) return
      try {
        setResult({ url, palette: extractPalette(image) })
      }
      catch (error) {
        appLogger.warning({
          source: 'ui',
          title: 'Artwork palette extraction failed',
          description: 'The artwork loaded, but its pixels could not be read for fullscreen colors.',
          context: {
            errorName: error && typeof error === 'object' && 'name' in error && typeof error.name === 'string' ? error.name : 'Unknown',
          },
        })
        setResult({ url, palette: FALLBACK_PALETTE })
      }
    }
    image.onerror = () => {
      if (cancelled) return
      appLogger.warning({
        source: 'ui',
        title: 'Artwork palette image failed to load',
        description: 'Fullscreen colors are using the neutral fallback because the palette image could not load.',
      })
      setResult({ url, palette: FALLBACK_PALETTE })
    }
    image.src = url

    return () => {
      cancelled = true
    }
  }, [url])

  return url && result?.url === url ? result.palette : FALLBACK_PALETTE
}
