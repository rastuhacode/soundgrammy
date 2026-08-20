import { useEffect, useRef, type RefObject } from 'react'

interface UseAudioPlaybackSyncOptions {
  audioRef: RefObject<HTMLAudioElement | null>
  trackId: number | null
  isPlaying: boolean
  listenAttemptEpoch: number
  isPlayingRef: RefObject<boolean>
  loadGenerationRef: RefObject<number>
  loadedTrackIdRef: RefObject<number | null>
  sourceErrorRef: RefObject<boolean>
  pendingSeekRef: RefObject<number | null>
  resumeAfterSeekRef: RefObject<boolean>
  retrySource: () => void
  handleSeek: (time: number) => void
  finishPendingSeek: (audio: HTMLAudioElement) => void
  playAudio: (audio: HTMLAudioElement, generation: number) => void
}

/** Keep requested store playback and same-track queue attempts on the element. */
export function useAudioPlaybackSync({
  audioRef,
  trackId,
  isPlaying,
  listenAttemptEpoch,
  isPlayingRef,
  loadGenerationRef,
  loadedTrackIdRef,
  sourceErrorRef,
  pendingSeekRef,
  resumeAfterSeekRef,
  retrySource,
  handleSeek,
  finishPendingSeek,
  playAudio,
}: UseAudioPlaybackSyncOptions) {
  const listenAttemptEpochRef = useRef(listenAttemptEpoch)

  useEffect(() => {
    const audio = audioRef.current
    if (!audio || trackId === null) return

    // Source setup is keyed by track id. After a failed setup, a normal Play
    // toggle must explicitly rebuild it; otherwise only changing tracks can
    // escape the paused/empty-buffer state.
    if (isPlaying && sourceErrorRef.current) {
      retrySource()
      return
    }

    if (!audio.src) return
    const generation = loadGenerationRef.current

    if (isPlaying && loadedTrackIdRef.current === trackId) {
      if (pendingSeekRef.current === null) {
        playAudio(audio, generation)
      }
      else {
        resumeAfterSeekRef.current = true
        finishPendingSeek(audio)
        // If still pending, discontinuity / canplay will resume — do not
        // force-land onto a distant or stale island.
      }
    }
    else {
      resumeAfterSeekRef.current = false
      audio.pause()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying])

  // Same track id, new queue row (duplicate / single-track skip): seek to start.
  useEffect(() => {
    if (listenAttemptEpochRef.current === listenAttemptEpoch) return
    listenAttemptEpochRef.current = listenAttemptEpoch

    const audio = audioRef.current
    if (!audio || trackId === null) return
    if (loadedTrackIdRef.current !== trackId) return

    handleSeek(0)
    finishPendingSeek(audio)
    if (
      isPlayingRef.current
      && audio.paused
      && pendingSeekRef.current === null
    ) {
      playAudio(audio, loadGenerationRef.current)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [listenAttemptEpoch])
}
