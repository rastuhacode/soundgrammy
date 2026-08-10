import { useCallback, useEffect, useRef, useState } from 'react'
import { registerPlaybackController } from '@/lib/playback-controller'
import { usePlayerStore } from '@/stores/player-store'
import { useRepeatStore } from '@/stores/repeat-store'
import { useAudioSeek } from './use-audio-seek'
import { useAudioSource } from './use-audio-source'
import { useAudioVolume } from './use-audio-volume'
import { useListenTracker } from './use-listen-tracker'
import { appLogger } from '@/lib/app-logger'

/**
 * WKWebView MSE advances `currentTime` as soon as buffered data exists, while
 * the audio output path for that MediaSource is still cold (~0.3–0.5s). The
 * clock races ahead of audible samples and the opening transient is skipped.
 * Cached `asset:` playback does not do this (AVFoundation keeps clock/output
 * aligned). Each new MediaSource is cold again, so we cannot warm once per
 * session. Mute + pin at 0 for this wall duration, then unmute in place.
 */
const MSE_COLD_START_PRIME_MS = 400

export function useAudioEngine() {
  const track = usePlayerStore(state => state.currentTrack)
  const isPlaying = usePlayerStore(state => state.isPlaying)
  const listenAttemptEpoch = usePlayerStore(state => state.listenAttemptEpoch)
  const setPlaying = usePlayerStore(state => state.setPlaying)
  const playNext = usePlayerStore(state => state.playNext)
  const repeat = useRepeatStore(state => state.repeat)

  const [currentTime, setCurrentTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const currentTimeRef = useRef(0)
  const audioRef = useRef<HTMLAudioElement>(null)
  const isPlayingRef = useRef(isPlaying)
  const loadGenerationRef = useRef(0)
  const loadedTrackIdRef = useRef<number | null>(null)
  /** Lets Play recover a source pipeline that failed without changing track. */
  const sourceErrorRef = useRef(false)
  const pendingSeekRef = useRef<number | null>(null)
  const resumeAfterSeekRef = useRef(false)
  const isSeekingRef = useRef(false)
  const listenAttemptEpochRef = useRef(listenAttemptEpoch)
  /** MSE cold-start pin-prime: idle → priming → done (per MediaSource attach). */
  const msePrimeRef = useRef<'idle' | 'priming' | 'done'>('idle')

  const {
    volume,
    applyVolume,
    audioRefCallback,
    handleVolumeChange,
    handleMuteToggle,
  } = useAudioVolume(audioRef)

  const playAudio = (audio: HTMLAudioElement, generation: number) => {
    audio
      .play()
      .then(() => {
        if (loadGenerationRef.current !== generation || !isPlayingRef.current) {
          audio.pause()
        }
      })
      .catch((error) => {
        if (loadGenerationRef.current === generation) {
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
        }
      })
  }

  const resetSeekRefs = () => {
    pendingSeekRef.current = null
    resumeAfterSeekRef.current = false
    isSeekingRef.current = false
  }

  const {
    downloadProgress,
    appendedBytes,
    bufferRevision,
    seekMseToTime,
    mseSnapToBufferedTime,
    mseLandToBufferedTime,
    isMseActive,
    streamingMse,
    retrySource,
    showInitialLoading,
    setShowInitialLoading,
  } = useAudioSource({
    audioRef,
    track,
    applyVolume,
    playAudio,
    isPlayingRef,
    pendingSeekRef,
    resumeAfterSeekRef,
    loadGenerationRef,
    loadedTrackIdRef,
    sourceErrorRef,
    resetSeekRefs,
    setCurrentTime,
    setDuration,
    setPlaying,
  })

  const {
    bufferedRanges,
    handleSeek,
    handleSeekStart,
    handleSeekEnd,
    finishPendingSeek,
    onLoadedMetadata,
    onSeeked,
    onCanPlay,
  } = useAudioSeek({
    audioRef,
    downloadProgress,
    appendedBytes,
    bufferRevision,
    seekMseToTime,
    mseSnapToBufferedTime,
    mseLandToBufferedTime,
    isMseActive,
    streamingMse,
    duration,
    currentTime,
    setCurrentTime,
    trackId: track?.id,
    isPlayingRef,
    loadedTrackIdRef,
    loadGenerationRef,
    pendingSeekRef,
    resumeAfterSeekRef,
    isSeekingRef,
    playAudio,
  })

  const { notifyCompleted } = useListenTracker({
    trackId: track?.id ?? null,
    durationSeconds: track?.duration,
    isPlaying,
  })

  const handleSeekRef = useRef(handleSeek)

  useEffect(() => {
    handleSeekRef.current = handleSeek
  }, [handleSeek])

  useEffect(() => {
    isPlayingRef.current = isPlaying
  }, [isPlaying])

  useEffect(() => {
    const audio = audioRef.current
    if (!audio) return
    msePrimeRef.current = 'idle'

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
        !audio.src.startsWith('blob:')
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
      audio.addEventListener('timeupdate', onPrimeTimeUpdate)
    }

    audio.addEventListener('playing', onPlaying)
    return () => {
      audio.removeEventListener('playing', onPlaying)
    }
  }, [track?.id])

  useEffect(() => {
    currentTimeRef.current = currentTime
  }, [currentTime])

  useEffect(() => {
    return registerPlaybackController({
      getCurrentTime: () => currentTimeRef.current,
      seekTo: (time) => {
        handleSeekRef.current(time)
      },
    })
  }, [])

  useEffect(() => {
    const audio = audioRef.current
    if (!audio || !track) return

    // Source setup is keyed by track id. After a failed setup, a normal Play
    // toggle must explicitly rebuild it; otherwise only changing tracks can
    // escape the paused/empty-buffer state.
    if (isPlaying && sourceErrorRef.current) {
      retrySource()
      return
    }

    if (!audio.src) return
    const generation = loadGenerationRef.current

    if (isPlaying && loadedTrackIdRef.current === track.id) {
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
  }, [isPlaying, setPlaying])

  // Same track id, new queue row (duplicate / single-track skip): seek to start.
  useEffect(() => {
    if (listenAttemptEpochRef.current === listenAttemptEpoch) return
    listenAttemptEpochRef.current = listenAttemptEpoch

    const audio = audioRef.current
    if (!audio || !track) return
    if (loadedTrackIdRef.current !== track.id) return

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

  const onTimeUpdate = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    if (isSeekingRef.current || pendingSeekRef.current !== null) return
    // Hold the UI at 0 while MSE pin-primes so the bar does not race ahead.
    if (msePrimeRef.current === 'priming') return
    setCurrentTime(event.currentTarget.currentTime)
  }

  const onDurationChange = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    const nextDuration = event.currentTarget.duration
    if (Number.isFinite(nextDuration) && nextDuration > 0) {
      setDuration(nextDuration)
    }
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

  function handleTrackEnded() {
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

  const getAudioElement = useCallback(() => audioRef.current, [])

  return {
    audioRefCallback,
    audioProps: {
      preload: 'metadata' as const,
      onLoadedMetadata,
      onSeeked,
      onCanPlay,
      onTimeUpdate,
      onDurationChange,
      onEnded: handleTrackEnded,
      onError: onMediaError,
    },
    currentTime,
    duration,
    bufferedRanges,
    showInitialLoading,
    volume,
    handleSeek,
    handleSeekStart,
    handleSeekEnd,
    handleVolumeChange,
    handleMuteToggle,
    getAudioElement,
  }
}
