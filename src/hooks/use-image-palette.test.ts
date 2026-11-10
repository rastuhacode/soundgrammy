import { describe, expect, it } from 'vitest'
import { artworkColorRange, completeArtworkPalette } from './use-image-palette'

describe('completeArtworkPalette', () => {
  it('uses three extracted artwork colors without changing them', () => {
    expect(completeArtworkPalette([
      [220, 40, 80],
      [40, 180, 100],
      [30, 80, 210],
    ])).toEqual([
      [220, 40, 80],
      [40, 180, 100],
      [30, 80, 210],
    ])
  })

  it('fills a two-color palette using only a blend of artwork colors', () => {
    expect(completeArtworkPalette([
      [200, 40, 20],
      [20, 80, 180],
    ])).toEqual([
      [200, 40, 20],
      [20, 80, 180],
      [110, 60, 100],
    ])
  })

  it('derives tonal variations for a single artwork color', () => {
    const palette = completeArtworkPalette([[32, 64, 96]])

    expect(palette).toHaveLength(3)
    expect(palette[0]).toEqual([32, 64, 96])
    expect(palette).not.toContainEqual([7, 89, 133])
  })

  it('leaves an empty extraction empty so the neutral fallback can be used', () => {
    expect(completeArtworkPalette([])).toEqual([])
  })
})

describe('artworkColorRange', () => {
  const parse = (tone: string) => tone.match(/[\d.]+/g)!.map(Number)
  const chroma = ([, saturation, lightness]: number[]) => saturation * (1 - Math.abs(2 * lightness / 100 - 1))

  it.each(['#c02040', '#04060c'])('increases the chroma sent to the color blend without shifting hue for %s', (color) => {
    const tones = artworkColorRange(color)
    const [rest, active, peak] = [tones.rest, tones.active, tones.peak].map(parse)
    expect(rest[0]).toBe(active[0])
    expect(active[0]).toBe(peak[0])
    expect(chroma(rest)).toBeLessThan(chroma(active))
    expect(chroma(active)).toBeLessThan(chroma(peak))
    expect(peak[1]).toBeLessThanOrEqual(100)
  })

  it('carries visible chroma into the blend even for a very dark swatch', () => {
    const dark = artworkColorRange('#04060c')
    expect(parse(dark.rest)[2]).toBeCloseTo(2, 1)
    expect(chroma(parse(dark.peak)) - chroma(parse(dark.rest))).toBeGreaterThanOrEqual(9)
  })

  it('strengthens muted recovered accents while preserving hue and vivid accents', () => {
    const muted = artworkColorRange('#6c7d87')
    const recovered = artworkColorRange('#6c7d87', { recoveredAccent: true })
    expect(recovered.rest).toBe(muted.rest)
    expect(parse(recovered.peak)[0]).toBe(parse(muted.peak)[0])
    expect(chroma(parse(recovered.peak))).toBeGreaterThan(chroma(parse(muted.peak)) * 1.3)

    const vivid = artworkColorRange('#5563a7')
    expect(artworkColorRange('#5563a7', { recoveredAccent: true }).peak).toBe(vivid.peak)
  })

  it('leaves black, gray, and white tones in the ambient gradient only', () => {
    for (const color of ['#000000', '#808080', '#ffffff']) {
      const tones = artworkColorRange(color)
      expect(parse(tones.rest)[1]).toBe(0)
      expect(tones.active).toBe('transparent')
      expect(tones.peak).toBe('transparent')
    }
    expect(parse(artworkColorRange('#000000').rest)[2]).toBe(0)
  })
})
