import { useEffect, useRef, useState } from 'react'
import { registerPlaybackController } from '@/lib/playback-controller'
import { usePlayerStore } from '@/stores/player-store'
import { useRepeatStore } from '@/stores/repeat-store'
import { useAudioSeek } from './use-audio-seek'
import { useAudioSource } from './use-audio-source'
import { useAudioVolume } from './use-audio-volume'

export function useAudioEngine() {
  const track = usePlayerStore(state => state.currentTrack)
  const isPlaying = usePlayerStore(state => state.isPlaying)
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
        const audio = audioRef.current
        if (audio) audio.currentTime = time
        currentTimeRef.current = time
        setCurrentTime(time)
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
      }
    }
    else {
      resumeAfterSeekRef.current = false
      audio.pause()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying, setPlaying])

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

  function handleTrackEnded() {
    if (repeat === 'one') {
      const audio = audioRef.current
      if (!audio) return
      audio.currentTime = 0
      return playAudio(audio, loadGenerationRef.current)
    }
    playNext()
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
