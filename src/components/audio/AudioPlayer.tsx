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
  const commandError = usePlayerStore(state => state.commandError)
  const isFullscreen = useFullscreenStore(state => state.isFullscreen)
  const exitFullscreen = useFullscreenStore(state => state.exitFullscreen)
  const {
    currentTime,
    duration,
    bufferedRanges,
    showInitialLoading,
    isSeeking,
    volume,
    handleSeek,
    handleSeekStart,
    handleSeekEnd,
    handleVolumeChange,
    handleMuteToggle,
    isActuallyPlaying,
  } = useAudioEngine()

  useEffect(() => {
    if (!track && isFullscreen) exitFullscreen()
  }, [exitFullscreen, isFullscreen, track])

  return (
    <>
      {commandError && <p role="alert" className="px-4 py-2 text-sm text-destructive">{commandError}</p>}
      {track && isFullscreen
        ? (
            <AudioFullscreenPlayer
              track={track}
              currentTime={currentTime}
              duration={duration}
              bufferedRanges={bufferedRanges}
              showInitialLoading={showInitialLoading}
              isSeeking={isSeeking}
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
              isSeeking={isSeeking}
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
