import { useEffect, useRef, type Dispatch, type RefObject, type SetStateAction } from 'react'

/**
 * WKWebView MSE advances `currentTime` as soon as buffered data exists, while
 * the audio output path for that MediaSource is still cold (~0.3–0.5s). The
 * clock races ahead of audible samples and the opening transient is skipped.
 * Cached `asset:` playback does not do this (AVFoundation keeps clock/output
 * aligned). Each new MediaSource is cold again, so we cannot warm once per
 * session. Mute + pin at 0 for this wall duration, then unmute in place.
 */
const MSE_COLD_START_PRIME_MS = 400

interface MediaPlaybackSyncState {
  trackId: number | null
  loadedTrackId: number | null
  pendingSeek: number | null
  isSeeking: boolean
  sourceFailed: boolean
}

/** Whether a native media event belongs to the stable, requested source. */
export function canSyncMediaPlaybackState(state: MediaPlaybackSyncState): boolean {
  return state.trackId !== null
    && state.loadedTrackId === state.trackId
    && state.pendingSeek === null
    && !state.isSeeking
    && !state.sourceFailed
}

interface UseMseColdStartPrimeOptions {
  audioRef: RefObject<HTMLAudioElement | null>
  trackId: number | null
  isPlayingRef: RefObject<boolean>
  loadGenerationRef: RefObject<number>
  loadedTrackIdRef: RefObject<number | null>
  pendingSeekRef: RefObject<number | null>
  isSeekingRef: RefObject<boolean>
  sourceErrorRef: RefObject<boolean>
  setCurrentTime: Dispatch<SetStateAction<number>>
}

/** Pin and mute each cold MSE source until WKWebView's output path is ready. */
export function useMseColdStartPrime({
  audioRef,
  trackId,
  isPlayingRef,
  loadGenerationRef,
  setCurrentTime,
}: UseMseColdStartPrimeOptions) {
  const msePrimeRef = useRef<'idle' | 'priming' | 'done'>('idle')

  useEffect(() => {
    const audio = audioRef.current
    if (!audio) return
    msePrimeRef.current = 'idle'
    let cleanupPrime: (() => void) | undefined

    const finishMsePrime = (generation: number, restoreMuted: boolean) => {
      if (msePrimeRef.current !== 'priming') return
      if (loadGenerationRef.current !== generation) {
        audio.muted = restoreMuted
        msePrimeRef.current = 'idle'
        return
      }
      try {
        audio.currentTime = 0
      }
      catch { /* ignore */ }
      audio.muted = restoreMuted
      msePrimeRef.current = 'done'
      setCurrentTime(0)
    }

    const onPlaying = () => {
      if (
        !isPlayingRef.current
        || !audio.src.startsWith('blob:')
        || msePrimeRef.current !== 'idle'
        || audio.currentTime > 0.05
      ) {
        return
      }
      msePrimeRef.current = 'priming'
      const generation = loadGenerationRef.current
      const restoreMuted = audio.muted
      const primeStartedAt = Date.now()
      audio.muted = true

      const onPrimeTimeUpdate = () => {
        if (msePrimeRef.current !== 'priming') {
          audio.removeEventListener('timeupdate', onPrimeTimeUpdate)
          return
        }
        if (!isPlayingRef.current) {
          audio.removeEventListener('timeupdate', onPrimeTimeUpdate)
          audio.muted = restoreMuted
          msePrimeRef.current = 'idle'
          return
        }
        // Keep the playhead at 0 so the cold window does not consume the intro.
        if (audio.currentTime > 0.02) {
          try {
            audio.currentTime = 0
          }
          catch { /* ignore */ }
        }
        if (Date.now() - primeStartedAt >= MSE_COLD_START_PRIME_MS) {
          audio.removeEventListener('timeupdate', onPrimeTimeUpdate)
          finishMsePrime(generation, restoreMuted)
        }
      }
      cleanupPrime?.()
      cleanupPrime = () => {
        audio.removeEventListener('timeupdate', onPrimeTimeUpdate)
        audio.muted = restoreMuted
      }
      audio.addEventListener('timeupdate', onPrimeTimeUpdate)
    }

    audio.addEventListener('playing', onPlaying)
    return () => {
      audio.removeEventListener('playing', onPlaying)
      cleanupPrime?.()
      msePrimeRef.current = 'idle'
    }
  }, [audioRef, isPlayingRef, loadGenerationRef, setCurrentTime, trackId])

  return msePrimeRef
}
