import { useEffect, useRef, useState, type RefObject } from 'react'
import { api } from '@/lib/api'
import {
  bounceEnergy,
  decodeProfileLane,
  motionTiming,
  sampleProfileLane,
  type BounceProfile,
} from '@/lib/bounce'
import { useFullscreenStore } from '@/stores/fullscreen-store'

const PROFILE_CROSSFADE_MS = 750

interface ArtworkBounceOptions {
  trackId: number
  elementRef: RefObject<HTMLDivElement | null>
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
  const { trackId, elementRef, currentTime, isActuallyPlaying } = options
  const timelineRef = useRef({ seconds: currentTime, observedAt: 0 })
  useEffect(() => {
    timelineRef.current = { seconds: currentTime, observedAt: performance.now() }
  }, [currentTime, isActuallyPlaying, trackId])
  const settings = useFullscreenStore(state => state.bounce)
  const reducedMotion = usePrefersReducedMotion()
  const [profileState, setProfileState] = useState<{
    trackId: number
    profile: BounceProfile
  } | null>(null)
  const profile = profileState?.trackId === trackId ? profileState.profile : null
  const profileReadyAtRef = useRef(0)
  const filteredEnergyRef = useRef(0)

  useEffect(() => {
    profileReadyAtRef.current = 0
    filteredEnergyRef.current = 0
    if (!settings.enabled || reducedMotion) return
    let current = true

    api.getTrackBounceProfile(trackId).then((response) => {
      if (!current || response.status !== 'ready') return
      const loudness = decodeProfileLane(response.loudnessData)
      const onset = decodeProfileLane(response.onsetData)
      if (loudness.length === 0 || loudness.length !== onset.length) return
      profileReadyAtRef.current = performance.now()
      setProfileState({
        trackId,
        profile: {
          algorithmVersion: response.algorithmVersion,
          frameMs: response.frameMs,
          durationMs: response.durationMs,
          loudness,
          onset,
        },
      })
    }).catch(() => {
      // Playback remains independent; provisional motion may continue.
    })

    return () => {
      current = false
    }
  }, [reducedMotion, settings.enabled, trackId])

  useEffect(() => {
    const element = elementRef.current
    if (!element) return
    if (!settings.enabled || reducedMotion) {
      element.style.transform = 'translate3d(0, 0, 0) scale(1)'
      element.style.willChange = 'auto'
      return
    }

    const { attackMs, releaseMs } = motionTiming(settings.smoothness)
    const strength = settings.strength / 100
    let filtered = filteredEnergyRef.current
    let lastFrameAt = performance.now()
    let frameId: number | null = null

    element.style.willChange = 'transform'

    const provisionalEnergy = (now: number) => {
      return (0.5 + 0.5 * Math.sin(now * Math.PI / 1000)) * 0.15
    }

    const render = (now: number) => {
      const timeline = timelineRef.current
      const seconds = timeline.seconds + (isActuallyPlaying ? Math.max(0, now - timeline.observedAt) / 1000 : 0)
      let target = 0
      if (isActuallyPlaying) {
        const provisional = provisionalEnergy(now)
        if (profile) {
          const loudness = sampleProfileLane(
            profile.loudness,
            seconds,
            profile.frameMs,
          )
          const onset = sampleProfileLane(profile.onset, seconds, profile.frameMs)
          const deterministic = bounceEnergy(loudness, onset, settings.balance)
          const crossfade = Math.min(
            1,
            Math.max(0, (now - profileReadyAtRef.current) / PROFILE_CROSSFADE_MS),
          )
          target = provisional * (1 - crossfade) + deterministic * crossfade
        }
        else {
          target = provisional
        }
      }

      const elapsed = Math.min(50, Math.max(0, now - lastFrameAt))
      lastFrameAt = now
      const timeConstant = target > filtered ? attackMs : releaseMs
      const alpha = 1 - Math.exp(-elapsed / timeConstant)
      filtered += (target - filtered) * alpha
      filteredEnergyRef.current = filtered

      const scale = 1 + filtered * 0.08 * strength
      const lift = filtered * 16 * strength
      element.style.transform = `translate3d(0, ${-lift}px, 0) scale(${scale})`

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
      element.style.willChange = 'auto'
    }
  }, [
    isActuallyPlaying,
    elementRef,
    profile,
    reducedMotion,
    settings.balance,
    settings.enabled,
    settings.smoothness,
    settings.strength,
    trackId,
  ])
}
