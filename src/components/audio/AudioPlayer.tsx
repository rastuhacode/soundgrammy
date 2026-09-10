import { useEffect } from 'react'
import { AudioEngineProvider } from '@/hooks/audio/engine-factory'
import { useAudioEngine } from '@/hooks/use-audio-engine'
import { useFullscreenStore } from '@/stores/fullscreen-store'
import { usePlayerStore } from '@/stores/player-store'
import { AudioFullscreenPlayer } from '../fullscreen/AudioFullscreenPlayer'
import { AudioPlayerBar } from './AudioPlayerBar'

export function AudioPlayer() {
  return <AudioEngineProvider><AudioPlayerContent /></AudioEngineProvider>
}

function AudioPlayerContent() {
  const track = usePlayerStore(state => state.currentTrack)
  const hydratePreferences = usePlayerStore(state => state.hydratePreferences)
  const isFullscreen = useFullscreenStore(state => state.isFullscreen)
  const exitFullscreen = useFullscreenStore(state => state.exitFullscreen)
  const {
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
    isActuallyPlaying,
  } = useAudioEngine()

  useEffect(hydratePreferences, [hydratePreferences])
  useEffect(() => {
    if (!track && isFullscreen) exitFullscreen()
  }, [exitFullscreen, isFullscreen, track])

  return (
    <>
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
              isActuallyPlaying={isActuallyPlaying}
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
