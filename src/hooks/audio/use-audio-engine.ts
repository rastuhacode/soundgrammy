import { useCallback, useState } from 'react'
import { useCachedThumbnail } from '@/hooks/use-cached-thumbnail'
import { usePlayerStore } from '@/stores/player-store'
import { useRepeatStore } from '@/stores/repeat-store'
import { useAudioPlaybackState } from './audio-playback-state'
import { useAudioControls } from './use-audio-controls'
import { useAudioMediaEvents } from './use-audio-media-events'
import { useAudioPlaybackSync } from './use-audio-playback-sync'
import { useAudioSeek } from './use-audio-seek'
import { useAudioSource } from './use-audio-source'
import { useAudioVolume } from './use-audio-volume'
import { useListenTracker } from './use-listen-tracker'
import { useMseColdStartPrime } from './use-mse-cold-start-prime'

export { isExpectedPlayInterruption } from './audio-playback-state'
export { registerMediaSessionActions } from './use-audio-controls'
export { canSyncMediaPlaybackState } from './use-mse-cold-start-prime'

export function useAudioEngine() {
  const track = usePlayerStore(state => state.currentTrack)
  const isPlaying = usePlayerStore(state => state.isPlaying)
  const listenAttemptEpoch = usePlayerStore(state => state.listenAttemptEpoch)
  const setPlaying = usePlayerStore(state => state.setPlaying)
  const playNext = usePlayerStore(state => state.playNext)
  const repeat = useRepeatStore(state => state.repeat)

  const [currentTime, setCurrentTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const { url: mediaArtworkUrl } = useCachedThumbnail(track?.id ?? 0, {
    enabled: track !== null,
  })

  const {
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
  } = useAudioPlaybackState({ isPlaying, setPlaying })

  const {
    volume,
    applyVolume,
    audioRefCallback,
    handleVolumeChange,
    handleMuteToggle,
  } = useAudioVolume(audioRef)

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

  useAudioControls({
    track,
    artworkUrl: mediaArtworkUrl,
    isPlaying,
    currentTime,
    duration,
    handleSeek,
  })

  const msePrimeRef = useMseColdStartPrime({
    audioRef,
    trackId: track?.id ?? null,
    isPlayingRef,
    loadGenerationRef,
    loadedTrackIdRef,
    pendingSeekRef,
    isSeekingRef,
    sourceErrorRef,
    setCurrentTime,
    setPlaying,
  })

  useAudioPlaybackSync({
    audioRef,
    trackId: track?.id ?? null,
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
  })

  const {
    onTimeUpdate,
    onDurationChange,
    onMediaPause,
    onMediaError,
    handleTrackEnded,
  } = useAudioMediaEvents({
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
  })

  const getAudioElement = useCallback(() => audioRef.current, [audioRef])

  return {
    audioRefCallback,
    audioProps: {
      preload: 'metadata' as const,
      onLoadedMetadata,
      onSeeked,
      onCanPlay,
      onTimeUpdate,
      onDurationChange,
      onPause: onMediaPause,
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
