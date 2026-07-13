import { useEffect, useState } from 'react'

const FALLBACK_PALETTE = ['#075985', '#172554', '#164e63']
const SAMPLE_SIZE = 48
const BUCKET_SIZE = 32
const MIN_COLOR_DISTANCE = 72

function toHex(value: number): string {
  return Math.max(0, Math.min(255, value)).toString(16).padStart(2, '0')
}

function colorDistance(a: number[], b: number[]): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2])
}

function extractPalette(image: HTMLImageElement): string[] {
  const canvas = document.createElement('canvas')
  canvas.width = SAMPLE_SIZE
  canvas.height = SAMPLE_SIZE
  const context = canvas.getContext('2d', { willReadFrequently: true })
  if (!context) return FALLBACK_PALETTE

  context.drawImage(image, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE)
  const pixels = context.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE).data
  const buckets = new Map<string, { color: number[], count: number }>()

  for (let index = 0; index < pixels.length; index += 4) {
    if (pixels[index + 3] < 180) continue
    const color = [
      Math.round(pixels[index] / BUCKET_SIZE) * BUCKET_SIZE,
      Math.round(pixels[index + 1] / BUCKET_SIZE) * BUCKET_SIZE,
      Math.round(pixels[index + 2] / BUCKET_SIZE) * BUCKET_SIZE,
    ].map(value => Math.min(255, value))
    const brightness = (color[0] + color[1] + color[2]) / 3
    if (brightness < 12 || brightness > 244) continue
    const key = color.join(',')
    const bucket = buckets.get(key)
    if (bucket) bucket.count += 1
    else buckets.set(key, { color, count: 1 })
  }

  const candidates = [...buckets.values()].sort((a, b) => b.count - a.count)
  const selected: number[][] = []
  for (const candidate of candidates) {
    if (selected.every(color => colorDistance(color, candidate.color) >= MIN_COLOR_DISTANCE)) {
      selected.push(candidate.color)
    }
    if (selected.length === 3) break
  }

  return FALLBACK_PALETTE.map((fallback, index) => {
    const color = selected[index]
    return color
      ? `#${toHex(color[0])}${toHex(color[1])}${toHex(color[2])}`
      : fallback
  })
}

export function useImagePalette(url: string | null): string[] {
  const [result, setResult] = useState<{
    url: string
    palette: string[]
  } | null>(null)

  useEffect(() => {
    if (!url) return

    let cancelled = false
    const image = new Image()
    image.decoding = 'async'
    image.onload = () => {
      if (cancelled) return
      try {
        setResult({ url, palette: extractPalette(image) })
      }
      catch {
        setResult({ url, palette: FALLBACK_PALETTE })
      }
    }
    image.onerror = () => {
      if (!cancelled) setResult({ url, palette: FALLBACK_PALETTE })
    }
    image.src = url

    return () => {
      cancelled = true
    }
  }, [url])

  return url && result?.url === url ? result.palette : FALLBACK_PALETTE
}
