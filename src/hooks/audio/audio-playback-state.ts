import { useEffect, useRef } from 'react'
import { appLogger } from '@/lib/app-logger'

export function isExpectedPlayInterruption(error: unknown): boolean {
  return Boolean(
    error
    && typeof error === 'object'
    && 'name' in error
    && error.name === 'AbortError',
  )
}

interface UseAudioPlaybackStateOptions {
  isPlaying: boolean
  setPlaying: (playing: boolean) => void
}

/** Shared mutable state for the source, seek, and media-event submodules. */
export function useAudioPlaybackState({
  isPlaying,
  setPlaying,
}: UseAudioPlaybackStateOptions) {
  const audioRef = useRef<HTMLAudioElement>(null)
  const isPlayingRef = useRef(isPlaying)
  const loadGenerationRef = useRef(0)
  const loadedTrackIdRef = useRef<number | null>(null)
  /** Lets Play recover a source pipeline that failed without changing track. */
  const sourceErrorRef = useRef(false)
  const pendingSeekRef = useRef<number | null>(null)
  const resumeAfterSeekRef = useRef(false)
  const isSeekingRef = useRef(false)

  useEffect(() => {
    isPlayingRef.current = isPlaying
  }, [isPlaying])

  const playAudio = (audio: HTMLAudioElement, generation: number) => {
    audio
      .play()
      .then(() => {
        if (loadGenerationRef.current !== generation || !isPlayingRef.current) {
          audio.pause()
        }
      })
      .catch((error) => {
        if (loadGenerationRef.current !== generation) return
        // pause(), load(), source replacement, and discontinuous seeks can all
        // reject an in-flight play() with AbortError. Those are expected player
        // control flow and must not poison/rebuild an otherwise healthy source.
        if (isExpectedPlayInterruption(error)) return

        sourceErrorRef.current = true
        appLogger.error({
          source: 'audio',
          title: 'Audio playback failed',
          description: 'The media element rejected the request to start playback.',
          error,
          context: {
            trackId: loadedTrackIdRef.current,
            generation,
            currentSrc: audio.currentSrc,
            networkState: audio.networkState,
            readyState: audio.readyState,
            errorCode: audio.error?.code ?? null,
            errorMessage: audio.error?.message ?? null,
          },
        })
        setPlaying(false)
      })
  }

  const resetSeekRefs = () => {
    pendingSeekRef.current = null
    resumeAfterSeekRef.current = false
    isSeekingRef.current = false
  }

  return {
    audioRef,
    isPlayingRef,
    loadGenerationRef,
    loadedTrackIdRef,
    sourceErrorRef,
    pendingSeekRef,
    resumeAfterSeekRef,
    isSeekingRef,
    playAudio,
    resetSeekRefs,
  }
}
