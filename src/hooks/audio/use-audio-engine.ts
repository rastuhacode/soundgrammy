import { useEffect, useRef, useState } from 'react'
import { registerPlaybackController } from '@/lib/playback-controller'
import { usePlayerStore } from '@/stores/player-store'
import { useRepeatStore } from '@/stores/repeat-store'
import { useAudioSeek } from './use-audio-seek'
import { useAudioSource } from './use-audio-source'
import { useAudioVolume } from './use-audio-volume'
import { useListenTracker } from './use-listen-tracker'

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
  const pendingSeekRef = useRef<number | null>(null)
  const resumeAfterSeekRef = useRef(false)
  const isSeekingRef = useRef(false)
  const listenAttemptEpochRef = useRef(listenAttemptEpoch)

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
      .catch(() => {
        if (loadGenerationRef.current === generation) {
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
    if (!audio || !track || !audio.src) return

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
  }
}
