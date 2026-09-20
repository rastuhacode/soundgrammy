import { describe, expect, it } from 'vitest'
import { completeArtworkPalette } from './use-image-palette'

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
