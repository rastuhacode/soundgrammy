import type {
  Dispatch,
  RefObject,
  SetStateAction,
  SyntheticEvent,
} from 'react'
import { appLogger } from '@/lib/app-logger'
import type { Track } from '@/lib/db'
import type { RepeatState } from '@/lib/repeat'
import { usePlayerStore } from '@/stores/player-store'
import { canSyncMediaPlaybackState } from './use-mse-cold-start-prime'

interface UseAudioMediaEventsOptions {
  audioRef: RefObject<HTMLAudioElement | null>
  track: Track | null
  repeat: RepeatState
  setCurrentTime: Dispatch<SetStateAction<number>>
  setDuration: Dispatch<SetStateAction<number>>
  setShowInitialLoading: Dispatch<SetStateAction<boolean>>
  setPlaying: (playing: boolean) => void
  playNext: (options: { reason: 'completed' }) => void
  notifyCompleted: (restartSameTrack?: boolean) => void
  isPlayingRef: RefObject<boolean>
  loadGenerationRef: RefObject<number>
  loadedTrackIdRef: RefObject<number | null>
  sourceErrorRef: RefObject<boolean>
  pendingSeekRef: RefObject<number | null>
  resumeAfterSeekRef: RefObject<boolean>
  isSeekingRef: RefObject<boolean>
  msePrimeRef: RefObject<'idle' | 'priming' | 'done'>
  handleSeek: (time: number) => void
  finishPendingSeek: (audio: HTMLAudioElement) => void
  playAudio: (audio: HTMLAudioElement, generation: number) => void
}

/** Build the `<audio>` event surface and own end/error lifecycle behavior. */
export function useAudioMediaEvents({
  audioRef,
  track,
  repeat,
  setCurrentTime,
  setDuration,
  setShowInitialLoading,
  setPlaying,
  playNext,
  notifyCompleted,
  isPlayingRef,
  loadGenerationRef,
  loadedTrackIdRef,
  sourceErrorRef,
  pendingSeekRef,
  resumeAfterSeekRef,
  isSeekingRef,
  msePrimeRef,
  handleSeek,
  finishPendingSeek,
  playAudio,
}: UseAudioMediaEventsOptions) {
  const onTimeUpdate = (event: SyntheticEvent<HTMLAudioElement>) => {
    if (isSeekingRef.current || pendingSeekRef.current !== null) return
    // Hold the UI at 0 while MSE pin-primes so the bar does not race ahead.
    if (msePrimeRef.current === 'priming') return
    setCurrentTime(event.currentTarget.currentTime)
  }

  const onDurationChange = (event: SyntheticEvent<HTMLAudioElement>) => {
    const nextDuration = event.currentTarget.duration
    if (Number.isFinite(nextDuration) && nextDuration > 0) {
      setDuration(nextDuration)
    }
  }

  const onMediaPause = (event: SyntheticEvent<HTMLAudioElement>) => {
    const audio = event.currentTarget
    if (
      !isPlayingRef.current
      || !audio.paused
      || audio.ended
      || !canSyncMediaPlaybackState({
        trackId: track?.id ?? null,
        loadedTrackId: loadedTrackIdRef.current,
        pendingSeek: pendingSeekRef.current,
        isSeeking: isSeekingRef.current,
        sourceFailed: sourceErrorRef.current,
      })
    ) {
      return
    }

    // Headphones and OS media controls pause the element directly. Keep the
    // player store (and all React playback UI/listen tracking) in sync.
    isPlayingRef.current = false
    setPlaying(false)
  }

  const onMediaError = () => {
    if (loadedTrackIdRef.current !== track?.id) return
    const audio = audioRef.current
    sourceErrorRef.current = true
    appLogger.error({
      source: 'audio',
      title: 'Media element failed',
      description: audio?.error?.message || 'The browser reported an audio media error.',
      context: {
        trackId: track?.id ?? null,
        generation: loadGenerationRef.current,
        currentSrc: audio?.currentSrc ?? '',
        networkState: audio?.networkState ?? null,
        readyState: audio?.readyState ?? null,
        errorCode: audio?.error?.code ?? null,
      },
    })
    setShowInitialLoading(false)
    pendingSeekRef.current = null
    resumeAfterSeekRef.current = false
    setPlaying(false)
  }

  const restartFromStart = (audio: HTMLAudioElement) => {
    handleSeek(0)
    finishPendingSeek(audio)
    if (
      isPlayingRef.current
      && audio.paused
      && pendingSeekRef.current === null
    ) {
      playAudio(audio, loadGenerationRef.current)
    }
  }

  const handleTrackEnded = () => {
    if (loadedTrackIdRef.current !== track?.id) return

    const audio = audioRef.current
    const metaDuration = track?.duration ?? 0
    // Mis-sniffed containers (e.g. WebM served as .mp3) can report a tiny
    // duration; with repeat-one that becomes an endless seek/Range spam loop.
    if (
      audio
      && metaDuration > 5
      && Number.isFinite(audio.duration)
      && audio.duration > 0
      && audio.duration < Math.min(2, metaDuration * 0.05)
    ) {
      onMediaError()
      return
    }
    if (repeat === 'one') {
      notifyCompleted(true)
      if (!audio) return
      restartFromStart(audio)
      return
    }

    // Same-track continue (e.g. adjacent duplicates / single-track repeat-all):
    // trackId will not change, so restart playback here after advancing cursor.
    // Last-track + repeat-none stops — a later play resumes via the isPlaying
    // effect in useListenTracker.
    const queue = usePlayerStore.getState().queue
    const isLast = queue.cursor === queue.tracks.length - 1
    const nextCursor = isLast ? 0 : queue.cursor + 1
    const nextTrackId = isLast && repeat === 'none'
      ? null
      : queue.tracks[nextCursor]?.id ?? null
    const restartSameTrack = nextTrackId != null && nextTrackId === track?.id

    notifyCompleted(restartSameTrack)
    playNext({ reason: 'completed' })
    if (restartSameTrack && audio) {
      restartFromStart(audio)
    }
  }

  return {
    onTimeUpdate,
    onDurationChange,
    onMediaPause,
    onMediaError,
    handleTrackEnded,
  }
}
