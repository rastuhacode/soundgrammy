import { useEffect } from 'react'
import { useAudioEngine } from '@/hooks/use-audio-engine'
import { useFullscreenStore } from '@/stores/fullscreen-store'
import { usePlayerStore } from '@/stores/player-store'
import { AudioFullscreenPlayer } from '../fullscreen/AudioFullscreenPlayer'
import { AudioPlayerBar } from './AudioPlayerBar'

export function AudioPlayer() {
  const track = usePlayerStore(state => state.currentTrack)
  const hydratePreferences = usePlayerStore(state => state.hydratePreferences)
  const isFullscreen = useFullscreenStore(state => state.isFullscreen)
  const exitFullscreen = useFullscreenStore(state => state.exitFullscreen)
  const {
    audioRefCallback,
    audioProps,
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
  } = useAudioEngine()

  useEffect(hydratePreferences, [hydratePreferences])
  useEffect(() => {
    if (!track && isFullscreen) exitFullscreen()
  }, [exitFullscreen, isFullscreen, track])

  return (
    <>
      <audio
        ref={audioRefCallback}
        className="hidden"
        {...audioProps}
      />

      {track && isFullscreen
        ? (
            <AudioFullscreenPlayer
              track={track}
              currentTime={currentTime}
              duration={duration}
              bufferedRanges={bufferedRanges}
              showInitialLoading={showInitialLoading}
              volume={volume}
              onVolumeChange={handleVolumeChange}
              onMuteToggle={handleMuteToggle}
              onSeek={handleSeek}
              onSeekStart={handleSeekStart}
              onSeekEnd={handleSeekEnd}
              getAudioElement={getAudioElement}
            />
          )
        : null}

      {track && !isFullscreen
        ? (
            <AudioPlayerBar
              track={track}
              currentTime={currentTime}
              duration={duration}
              bufferedRanges={bufferedRanges}
              showInitialLoading={showInitialLoading}
              volume={volume}
              onSeek={handleSeek}
              onSeekStart={handleSeekStart}
              onSeekEnd={handleSeekEnd}
              onVolumeChange={handleVolumeChange}
              onMuteToggle={handleMuteToggle}
            />
          )
        : null}
    </>
  )
}
