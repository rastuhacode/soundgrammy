// @vitest-environment jsdom
import { act, createElement, useEffect, useRef } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { useFullscreenStore } from '@/stores/fullscreen-store'
import type { BounceProfileResponse } from '@/types'
import { useArtworkBounce } from './use-artwork-bounce'

vi.mock('@/lib/api', () => ({ api: { getTrackBounceProfile: vi.fn() } }))
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: vi.fn() }))

let root: Root
let container: HTMLDivElement
let now: number
let frames: Map<number, FrameRequestCallback>
let nextFrame: number
let reduced: boolean

function Harness({ trackId = 1, currentTime = 0, playing = true }: { trackId?: number, currentTime?: number, playing?: boolean }) {
  const elementRef = useRef<HTMLDivElement>(null)
  const paletteRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const artwork = document.createElement('div')
    const palette = document.createElement('div')
    artwork.dataset.artwork = 'true'
    palette.dataset.palette = 'true'
    elementRef.current = artwork
    paletteRef.current = palette
    container.append(artwork, palette)
    return () => {
      artwork.remove()
      palette.remove()
    }
  }, [])
  useArtworkBounce({ trackId, currentTime, isActuallyPlaying: playing, elementRef, paletteRef })
  return null
}

function ready(): BounceProfileResponse {
  const lane = (value: number) => btoa(String.fromCharCode(...new Uint8Array(400).fill(value)))
  return { status: 'ready', algorithmVersion: 2, frameMs: 25, durationMs: 10_000, loudnessData: lane(220), onsetData: lane(150), bassData: lane(255), midData: lane(0), trebleData: lane(0) }
}

async function mount(props = {}) {
  await act(async () => {
    root.render(createElement(Harness, props))
  })
}

function advance(milliseconds: number) {
  const end = now + milliseconds
  while (now < end) {
    now += 16
    const callbacks = [...frames.values()]
    frames.clear()
    callbacks.forEach(callback => callback(now))
  }
}

const artwork = () => container.querySelector<HTMLElement>('[data-artwork]')!
const palette = () => container.querySelector<HTMLElement>('[data-palette]')!
const energy = (band: string) => Number(palette().style.getPropertyValue(`--fullscreen-${band}-energy`))
const spread = (band: string) => Number(palette().style.getPropertyValue(`--fullscreen-${band}-spread`))

beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  reduced = false
  now = 1000
  frames = new Map()
  nextFrame = 0
  vi.spyOn(performance, 'now').mockImplementation(() => now)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    frames.set(++nextFrame, callback)
    return nextFrame
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id))
  vi.stubGlobal('matchMedia', () => ({ matches: reduced, addEventListener: vi.fn(), removeEventListener: vi.fn() }))
  useFullscreenStore.setState({ bounce: { enabled: true } })
  vi.mocked(api.getTrackBounceProfile).mockResolvedValue(ready())
  container = document.createElement('div')
  document.body.append(container)
  root = createRoot(container)
})

afterEach(async () => {
  await act(async () => root.unmount())
  expect(frames.size).toBe(0)
  container.remove()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  vi.clearAllMocks()
})

describe('fullscreen dynamic lifecycle', () => {
  it('routes bass to the lower palette, settles on pause, and resets when disabled', async () => {
    await mount()
    advance(1200)
    expect(energy('bass')).toBeGreaterThan(0.1)
    expect(energy('mid')).toBe(0)
    expect(energy('treble')).toBe(0)
    expect(artwork().style.transform).not.toContain('scale(1)')
    await mount({ currentTime: 1.2, playing: false })
    advance(2500)
    expect(energy('bass')).toBeLessThan(0.001)
    await act(async () => useFullscreenStore.getState().setBounceEnabled(false))
    expect(artwork().style.transform).toContain('scale(1)')
    expect(energy('bass')).toBe(0)
    expect(spread('bass')).toBe(0)
    expect(frames.size).toBe(0)
  })

  it('keeps artwork stationary while analysis is unavailable', async () => {
    vi.mocked(api.getTrackBounceProfile).mockResolvedValue({ status: 'unavailable' })
    await mount()
    advance(2000)
    expect(artwork().style.transform).toContain('scale(1)')
    expect(energy('bass')).toBe(0)
    expect(frames.size).toBe(0)
  })

  it('ignores stale analysis after switching tracks', async () => {
    let resolve!: (response: BounceProfileResponse) => void
    vi.mocked(api.getTrackBounceProfile).mockReturnValueOnce(new Promise(done => (resolve = done)))
    await mount()
    vi.mocked(api.getTrackBounceProfile).mockResolvedValue({ status: 'unavailable' })
    await mount({ trackId: 2 })
    await act(async () => resolve(ready()))
    advance(1200)
    expect(energy('bass')).toBe(0)
    expect(frames.size).toBe(0)
  })

  it('respects reduced motion without requesting analysis', async () => {
    reduced = true
    await mount()
    expect(api.getTrackBounceProfile).not.toHaveBeenCalled()
    expect(frames.size).toBe(0)
    expect(energy('bass')).toBe(0)
  })

  it('rejects incomplete profile lanes', async () => {
    const response = ready()
    if (response.status === 'ready') response.bassData = ''
    vi.mocked(api.getTrackBounceProfile).mockResolvedValue(response)
    await mount()
    expect(frames.size).toBe(0)
  })

  it('clears old energy when seeking into a silent section', async () => {
    const response = ready()
    if (response.status === 'ready') {
      const values = new Uint8Array(400).fill(255, 0, 80)
      const data = btoa(String.fromCharCode(...values))
      response.loudnessData = response.onsetData = response.bassData = data
    }
    vi.mocked(api.getTrackBounceProfile).mockResolvedValue(response)
    await mount()
    advance(1200)
    expect(energy('bass')).toBeGreaterThan(0.1)
    await mount({ currentTime: 5 })
    advance(16)
    expect(energy('bass')).toBe(0)
    expect(artwork().style.transform).toContain('scale(1)')
    expect(spread('bass')).toBe(0)
  })

  it('releases energy after the analyzed track ends', async () => {
    await mount({ currentTime: 8 })
    advance(1200)
    expect(energy('bass')).toBeGreaterThan(0.1)
    advance(3500)
    expect(energy('bass')).toBeLessThan(0.001)
  })

  it('lets simultaneous color fields linger and mix between hits, then settle', async () => {
    const response = ready()
    if (response.status === 'ready') {
      const values = new Uint8Array(400).fill(255, 0, 80)
      const data = btoa(String.fromCharCode(...values))
      response.bassData = response.midData = response.trebleData = data
    }
    vi.mocked(api.getTrackBounceProfile).mockResolvedValue(response)
    await mount()
    advance(2500)
    for (const band of ['bass', 'mid', 'treble']) {
      expect(energy(band)).toBeGreaterThan(0.03)
    }
    advance(3000)
    for (const band of ['bass', 'mid', 'treble']) {
      expect(energy(band)).toBeLessThan(0.03)
    }
  })

  it('lets a band hit spread outward after its color pulse starts to fade', async () => {
    const response = ready()
    if (response.status === 'ready') {
      const values = new Uint8Array(400).fill(180)
      values.fill(255, 80, 83)
      response.bassData = btoa(String.fromCharCode(...values))
    }
    vi.mocked(api.getTrackBounceProfile).mockResolvedValue(response)
    await mount({ currentTime: 1 })
    advance(900)
    const sustained = energy('bass')
    advance(144)
    const hit = energy('bass')
    const initialSpread = spread('bass')
    expect(hit).toBeGreaterThan(sustained + 0.3)
    expect(energy('mid')).toBe(0)
    expect(energy('treble')).toBe(0)
    expect(spread('mid')).toBe(0)
    expect(spread('treble')).toBe(0)
    advance(112)
    expect(energy('bass')).toBeLessThan(hit)
    expect(spread('bass')).toBeGreaterThan(initialSpread)
    advance(388)
    expect(energy('bass')).toBeLessThan(hit - 0.2)
    expect(energy('bass')).toBeGreaterThan(sustained)
  })

  it('retains most of a quiet hit’s color pulse while prioritizing loud artwork hits', async () => {
    const response = ready()
    if (response.status === 'ready') {
      const loudness = new Uint8Array(400).fill(120)
      loudness.fill(200, 120)
      const bass = new Uint8Array(400).fill(180)
      bass.fill(255, 80, 83)
      bass.fill(255, 200, 203)
      response.loudnessData = btoa(String.fromCharCode(...loudness))
      response.bassData = btoa(String.fromCharCode(...bass))
    }
    vi.mocked(api.getTrackBounceProfile).mockResolvedValue(response)
    const lift = () => Math.abs(Number.parseFloat(artwork().style.transform.split(',')[1]))

    await mount({ currentTime: 1 })
    advance(1056)
    const quiet = { lift: lift(), color: energy('bass'), spread: spread('bass') }
    expect(quiet.color).toBeGreaterThan(0.1)

    await mount({ currentTime: 4 })
    advance(1056)
    expect(lift()).toBeGreaterThan(quiet.lift * 2)
    expect(quiet.color).toBeGreaterThan(energy('bass') * 0.8)
    expect(quiet.spread).toBeGreaterThan(spread('bass') * 0.8)
    expect(energy('bass')).toBeGreaterThan(quiet.color)
  })

  it('gives upper bands less lift and a slower response than bass', async () => {
    const response = ready()
    if (response.status === 'ready') {
      const withHit = (frame: number) => {
        const values = new Uint8Array(400).fill(100)
        values.fill(255, frame, frame + 3)
        return btoa(String.fromCharCode(...values))
      }
      response.trebleData = withHit(80)
      response.midData = withHit(160)
      response.bassData = withHit(240)
    }
    vi.mocked(api.getTrackBounceProfile).mockResolvedValue(response)
    const lift = () => Math.abs(Number.parseFloat(artwork().style.transform.split(',')[1]))

    await mount({ currentTime: 1 })
    advance(1056)
    expect(energy('treble')).toBeGreaterThan(0.4)
    const trebleLift = lift()
    expect(trebleLift).toBeGreaterThan(0)
    advance(160)
    const trebleTail = lift() / trebleLift

    await mount({ currentTime: 3 })
    advance(1056)
    const midLift = lift()
    expect(midLift).toBeGreaterThan(trebleLift * 2)

    await mount({ currentTime: 5 })
    advance(1056)
    const bassLift = lift()
    expect(bassLift).toBeGreaterThan(midLift * 2.5)
    advance(160)
    expect(lift() / bassLift).toBeLessThan(trebleTail)
  })

  it('lets mid hits take over during sustained bass and restores bass priority after a kick', async () => {
    const response = ready()
    if (response.status === 'ready') {
      const bass = new Uint8Array(400).fill(180)
      const mid = new Uint8Array(400).fill(100)
      bass.fill(255, 128, 131)
      for (const frame of [38, 90, 146]) mid.fill(255, frame, frame + 3)
      response.bassData = btoa(String.fromCharCode(...bass))
      response.midData = btoa(String.fromCharCode(...mid))
    }
    vi.mocked(api.getTrackBounceProfile).mockResolvedValue(response)
    const lift = () => Math.abs(Number.parseFloat(artwork().style.transform.split(',')[1]))

    await mount()
    advance(1008)
    const earlyHit = lift()
    advance(1296)
    const lateHit = lift()
    expect(lateHit).toBeGreaterThan(earlyHit * 1.2)
    advance(1408)
    expect(lift()).toBeLessThan(lateHit)
  })
})
