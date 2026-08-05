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
import { usePlayerStore } from '@/stores/player-store'

const PROFILE_CROSSFADE_MS = 750
const PROVISIONAL_SAMPLE_MS = 50

interface ArtworkBounceOptions {
  trackId: number
  elementRef: RefObject<HTMLDivElement | null>
  getAudioElement: () => HTMLAudioElement | null
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

interface ProvisionalSampler {
  sample: (now: number) => number
  dispose: () => void
}

function createProvisionalSampler(audio: HTMLAudioElement): ProvisionalSampler | null {
  type CapturableAudio = HTMLAudioElement & { captureStream?: () => MediaStream }
  const captureStream = (audio as CapturableAudio).captureStream
  if (typeof captureStream !== 'function') return null

  try {
    const stream = captureStream.call(audio)
    if (stream.getAudioTracks().length === 0) return null
    const context = new AudioContext()
    const source = context.createMediaStreamSource(stream)
    const analyser = context.createAnalyser()
    analyser.fftSize = 512
    analyser.smoothingTimeConstant = 0.35
    source.connect(analyser)
    void context.resume().catch(() => {})

    const timeData = new Float32Array(analyser.fftSize)
    const frequencyData = new Uint8Array(analyser.frequencyBinCount)
    const previousFrequency = new Uint8Array(analyser.frequencyBinCount)
    let previousSampleAt = -Infinity
    let cached = 0
    let slowRms = 0.01

    return {
      sample: (now) => {
        if (now - previousSampleAt < PROVISIONAL_SAMPLE_MS) return cached
        previousSampleAt = now
        analyser.getFloatTimeDomainData(timeData)
        analyser.getByteFrequencyData(frequencyData)
        let power = 0
        for (const value of timeData) power += value * value
        const rms = Math.sqrt(power / timeData.length)
        slowRms += (rms - slowRms) * 0.08

        let flux = 0
        for (let index = 2; index < frequencyData.length; index += 1) {
          flux += Math.max(0, (frequencyData[index] ?? 0) - (previousFrequency[index] ?? 0))
          previousFrequency[index] = frequencyData[index] ?? 0
        }
        const relativeLevel = Math.min(1, rms / Math.max(0.02, slowRms * 2.5))
        const relativeFlux = Math.min(1, flux / (frequencyData.length * 18))
        cached = Math.min(0.3, relativeLevel * 0.18 + relativeFlux * 0.12)
        return cached
      },
      dispose: () => {
        source.disconnect()
        analyser.disconnect()
        void context.close().catch(() => {})
      },
    }
  }
  catch {
    return null
  }
}

export function useArtworkBounce(options: ArtworkBounceOptions): void {
  const { trackId, elementRef, getAudioElement } = options
  const settings = useFullscreenStore(state => state.bounce)
  const isPlaying = usePlayerStore(state => state.isPlaying)
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

    void api.getTrackBounceProfile(trackId).then((response) => {
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

    const audio = getAudioElement()
    const sampler = audio ? createProvisionalSampler(audio) : null
    const { attackMs, releaseMs } = motionTiming(settings.smoothness)
    const strength = settings.strength / 100
    let filtered = filteredEnergyRef.current
    let lastFrameAt = performance.now()
    let frameId: number | null = null

    element.style.willChange = 'transform'

    const provisionalEnergy = (now: number) => {
      if (sampler) return sampler.sample(now)
      return (0.5 + 0.5 * Math.sin(now * Math.PI / 1000)) * 0.15
    }

    const render = (now: number) => {
      const currentAudio = getAudioElement()
      let target = 0
      if (isPlaying && currentAudio && !currentAudio.paused && !currentAudio.ended) {
        const provisional = provisionalEnergy(now)
        if (profile) {
          const loudness = sampleProfileLane(
            profile.loudness,
            currentAudio.currentTime,
            profile.frameMs,
          )
          const onset = sampleProfileLane(profile.onset, currentAudio.currentTime, profile.frameMs)
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
      sampler?.dispose()
      element.style.willChange = 'auto'
    }
  }, [
    isPlaying,
    elementRef,
    getAudioElement,
    profile,
    reducedMotion,
    settings.balance,
    settings.enabled,
    settings.smoothness,
    settings.strength,
    trackId,
  ])
}
