import { useEffect, useRef, useState, type RefObject } from 'react'
import { api } from '@/lib/api'
import {
  adaptBounceProfile,
  bandTransients,
  bassMotionTarget,
  bassMotionTiming,
  decodeProfileLane,
  followEnergy,
  onsetDensity,
  sampleProfileLane,
  type BounceProfile,
} from '@/lib/bounce'
import { useFullscreenStore } from '@/stores/fullscreen-store'

const PROFILE_CROSSFADE_MS = 750
const PALETTE_BANDS = ['bass', 'mid', 'treble'] as const
const PALETTE_BLOOM_TIMING = {
  bass: { attack: 160, release: 880 },
  mid: { attack: 125, release: 720 },
  treble: { attack: 90, release: 500 },
}
// A gentle high cut across the three analysis bands. These are visual weights,
// with progressively slower envelopes, rather than a filter on the audio.
const MID_MOTION_GAIN = 0.33
const TREBLE_MOTION_GAIN = 0.13
const BASS_ATTACK_THRESHOLD = 0.20
const BASS_HANDOFF_DELAY_MS = 550
const BASS_HANDOFF_RAMP_MS = 850

interface ArtworkBounceOptions {
  trackId: number
  elementRef: RefObject<HTMLDivElement | null>
  paletteRef: RefObject<HTMLDivElement | null>
  currentTime: number
  isActuallyPlaying: boolean
}

function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(() => (
    typeof window !== 'undefined'
    && typeof window.matchMedia === 'function'
    && window.matchMedia('(prefers-reduced-motion: reduce)').matches
  ))

  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return
    const query = window.matchMedia('(prefers-reduced-motion: reduce)')
    const update = () => setReduced(query.matches)
    query.addEventListener('change', update)
    return () => query.removeEventListener('change', update)
  }, [])

  return reduced
}

export function useArtworkBounce(options: ArtworkBounceOptions) {
  const { trackId, elementRef, paletteRef, currentTime, isActuallyPlaying } = options
  const timelineRef = useRef({ seconds: currentTime, observedAt: 0, revision: 0, playing: isActuallyPlaying })
  useEffect(() => {
    const previous = timelineRef.current
    const now = performance.now()
    const expected = previous.seconds + (previous.playing && previous.observedAt > 0 ? (now - previous.observedAt) / 1000 : 0)
    timelineRef.current = {
      seconds: currentTime,
      observedAt: now,
      revision: previous.revision + (Math.abs(currentTime - expected) > 0.5 ? 1 : 0),
      playing: isActuallyPlaying,
    }
  }, [currentTime, isActuallyPlaying, trackId])
  const enabled = useFullscreenStore(state => state.bounce.enabled)
  const reducedMotion = usePrefersReducedMotion()
  const [profileState, setProfileState] = useState<{
    trackId: number
    profile: BounceProfile
    loudnessReference: number
    paletteAccentBoosts: number[]
    density: Uint8Array
    bassDensity: Uint8Array
    transients: Uint8Array[]
    readyAt: number
  } | null>(null)
  const activeProfile = profileState?.trackId === trackId ? profileState : null

  useEffect(() => {
    if (!enabled || reducedMotion) return
    let current = true

    api.getTrackBounceProfile(trackId).then((response) => {
      if (!current || response.status !== 'ready') return
      if (!Number.isFinite(response.frameMs) || response.frameMs <= 0
        || !Number.isFinite(response.durationMs) || response.durationMs <= 0) return
      const loudness = decodeProfileLane(response.loudnessData)
      const onset = decodeProfileLane(response.onsetData)
      const bass = decodeProfileLane(response.bassData)
      const mid = decodeProfileLane(response.midData)
      const treble = decodeProfileLane(response.trebleData)
      if (loudness.length === 0 || [onset, bass, mid, treble].some(lane => lane.length !== loudness.length)) return
      const transients = [bass, mid, treble].map(lane => bandTransients(lane, response.frameMs))
      const profile = { algorithmVersion: response.algorithmVersion, frameMs: response.frameMs, durationMs: response.durationMs, loudness, onset, bass, mid, treble }
      setProfileState({
        trackId,
        readyAt: performance.now(),
        ...adaptBounceProfile(profile, transients),
        density: onsetDensity(onset, response.frameMs),
        bassDensity: onsetDensity(transients[0], response.frameMs),
        transients,
        profile,
      })
    }).catch(() => {
      // Analysis is optional. Keep the ambient palette, with no synthetic beats.
    })

    return () => {
      current = false
    }
  }, [enabled, reducedMotion, trackId])

  useEffect(() => {
    const element = elementRef.current
    const palette = paletteRef.current
    if (!element || !palette) return
    const reset = () => {
      element.style.transform = 'translate3d(0, 0, 0) scale(1)'
      element.style.willChange = 'auto'
      for (const band of PALETTE_BANDS) {
        palette.style.setProperty(`--fullscreen-${band}-energy`, '0')
        palette.style.setProperty(`--fullscreen-${band}-spread`, '0')
      }
    }
    reset()
    if (!enabled || reducedMotion || !activeProfile) return

    let body = 0
    let bassImpact = 0
    let midImpact = 0
    let trebleImpact = 0
    let bassQuietMs = 0
    let upperHandoff = 0
    const bandBody = [0, 0, 0]
    const bandPulse = [0, 0, 0]
    const bandTrail = [0, 0, 0]
    let revision = timelineRef.current.revision
    let lastFrameAt = performance.now()
    let frameId: number | null = null
    const { profile, loudnessReference, paletteAccentBoosts, density, bassDensity, transients, readyAt } = activeProfile
    const sample = (lane: Uint8Array, seconds: number) => sampleProfileLane(lane, seconds, profile.frameMs)
    element.style.willChange = 'transform'

    const render = (now: number) => {
      const timeline = timelineRef.current
      const seconds = timeline.seconds + (timeline.playing ? Math.max(0, now - timeline.observedAt) / 1000 : 0)
      const playing = timeline.playing && seconds < profile.durationMs / 1000
      const fade = Math.min(1, Math.max(0, (now - readyAt) / PROFILE_CROSSFADE_MS))
      // A steeper curve separates quieter hits from loud passages. Let near-peak
      // sections reach full strength without needing an exact peak frame.
      const nearPeak = playing ? Math.min(1, sample(profile.loudness, seconds) / loudnessReference) : 0
      const volumeGain = nearPeak ** 4 * fade
      // The band signals carry the dynamics. Overall loudness only nudges the palette.
      const paletteGain = (0.82 + 0.18 * nearPeak) * fade
      const activity = sample(density, seconds)
      const bassActivity = sample(bassDensity, seconds)
      const timing = bassMotionTiming(bassActivity)
      if (revision !== timeline.revision) {
        // A seek must not carry the previous section's visual energy with it.
        body = bassImpact = midImpact = trebleImpact = 0
        bassQuietMs = upperHandoff = 0
        bandBody.fill(0)
        bandPulse.fill(0)
        bandTrail.fill(0)
        revision = timeline.revision
      }
      const elapsed = Math.min(50, Math.max(0, now - lastFrameAt))
      lastFrameAt = now
      // Accents carry most of the response. A quieter body and lingering trail
      // retain the atmosphere without pinning dense masters at full expansion.
      const paletteRelease = playing ? 1 : 0.3
      for (const [index, band] of PALETTE_BANDS.entries()) {
        const level = playing ? sample(profile[band], seconds) * paletteGain : 0
        const attack = playing ? sample(transients[index], seconds) : 0
        const accent = (attack + paletteAccentBoosts[index] * nearPeak ** 2 * attack * (1 - attack)) * paletteGain
        bandBody[index] = followEnergy(bandBody[index], level, elapsed, 120, 700 * paletteRelease)
        bandPulse[index] = followEnergy(bandPulse[index], accent, elapsed, 14, (180 - activity * 50) * paletteRelease)
        const bloom = PALETTE_BLOOM_TIMING[band]
        bandTrail[index] = followEnergy(bandTrail[index], bandPulse[index], elapsed, bloom.attack, bloom.release * paletteRelease)
      }
      // Kick and bass get the fast movement; upper bands add a softer motion.
      const bassLevel = playing ? sample(profile.bass, seconds) : 0
      const midLevel = playing ? sample(profile.mid, seconds) : 0
      const bassAttack = playing ? sample(transients[0], seconds) : 0
      const midAttack = playing ? sample(transients[1], seconds) : 0
      const trebleAttack = playing ? sample(transients[2], seconds) : 0
      // Sustained bass can stay loud between kicks. Let upper attacks take
      // over gradually during a gap, then give priority back to the next kick.
      bassQuietMs = playing && bassAttack < BASS_ATTACK_THRESHOLD
        ? Math.min(BASS_HANDOFF_DELAY_MS + BASS_HANDOFF_RAMP_MS, bassQuietMs + elapsed)
        : 0
      const handoffTarget = Math.max(0, (bassQuietMs - BASS_HANDOFF_DELAY_MS) / BASS_HANDOFF_RAMP_MS)
      upperHandoff = followEnergy(upperHandoff, handoffTarget, elapsed, 100, 70)
      body = followEnergy(body, volumeGain * Math.max(bassLevel, midLevel * 0.12), elapsed, 180, 550)
      bassImpact = followEnergy(bassImpact, bassMotionTarget(volumeGain, bassAttack, bassLevel, bassActivity), elapsed, timing.attackMs, timing.releaseMs)
      midImpact = followEnergy(midImpact, volumeGain * midAttack * (MID_MOTION_GAIN + 0.27 * upperHandoff), elapsed, 65, 275)
      trebleImpact = followEnergy(trebleImpact, volumeGain * trebleAttack * (TREBLE_MOTION_GAIN + 0.09 * upperHandoff), elapsed, 95, 330)
      const impact = bassImpact + (1 - bassImpact) ** 2 * Math.max(midImpact, trebleImpact)
      // Color registers the hit first; its softer spatial bloom follows. When
      // the bands overlap, their tails open up the whole field a little more.
      const spill = Math.min(...bandTrail) * 0.24
      for (const [index, band] of PALETTE_BANDS.entries()) {
        const color = bandBody[index] * 0.14 + bandPulse[index] * 0.8 + bandTrail[index] * 0.22
        // Keep headroom for the strongest hits instead of flattening their peaks.
        const energy = color - Math.max(0, color - 0.74) * (0.16 / 0.42)
        const spread = Math.min(1, bandBody[index] * 0.15 + bandPulse[index] * 0.1 + bandTrail[index] + spill)
        palette.style.setProperty(`--fullscreen-${band}-energy`, energy.toFixed(4))
        palette.style.setProperty(`--fullscreen-${band}-spread`, spread.toFixed(4))
      }
      element.style.transform = `translate3d(0, ${-impact * 6.1}px, 0) scale(${1 + body * 0.02 + impact * 0.055})`
      if (!document.hidden) frameId = requestAnimationFrame(render)
    }

    const onVisibilityChange = () => {
      if (document.hidden) {
        if (frameId !== null) cancelAnimationFrame(frameId)
        frameId = null
      }
      else if (frameId === null) {
        lastFrameAt = performance.now()
        frameId = requestAnimationFrame(render)
      }
    }

    document.addEventListener('visibilitychange', onVisibilityChange)
    if (!document.hidden) frameId = requestAnimationFrame(render)
    return () => {
      document.removeEventListener('visibilitychange', onVisibilityChange)
      if (frameId !== null) cancelAnimationFrame(frameId)
      reset()
    }
  }, [elementRef, paletteRef, activeProfile, reducedMotion, enabled, trackId])
}
