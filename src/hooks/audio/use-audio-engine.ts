import { useCallback, useEffect, useRef, useSyncExternalStore } from 'react'
import { usePlayerStore } from '@/stores/player-store'
import { useAudioControls } from './use-audio-controls'
import { useAudioVolume } from './use-audio-volume'
import { useListenTracker } from './use-listen-tracker'
import { useSelectedAudioEngine } from './engine-factory'
import { connectPlayerEngine } from './player-integration'

export function useAudioEngine() {
  const engine = useSelectedAudioEngine()
  const subscribe = useCallback((notify: () => void) => engine.subscribe(notify), [engine])
  const snapshot = useSyncExternalStore(subscribe, engine.getSnapshot.bind(engine))
  const track = usePlayerStore(state => state.currentTrack)
  const activity = useListenTracker({ trackId: track?.id ?? null, durationSeconds: track?.duration })
  const activityRef = useRef(activity)
  useEffect(() => {
    activityRef.current = activity
  })
  useEffect(() => connectPlayerEngine(engine, {
    notifyPlaying: () => activityRef.current.notifyPlaying(),
    notifyActivityStopped: () => activityRef.current.notifyActivityStopped(),
    notifyCompleted: restart => activityRef.current.notifyCompleted(restart),
  }), [engine])
  const handleSeek = useCallback((seconds: number) => {
    if (Number.isFinite(seconds)) void engine.seek(seconds)
  }, [engine])
  const handleSeekStart = useCallback(() => {
    void engine.beginSeek()
  }, [engine])
  const handleSeekEnd = useCallback(() => {
    void engine.endSeek()
  }, [engine])
  const volume = useAudioVolume(engine)
  useAudioControls({
    currentTime: snapshot.currentTimeSeconds, duration: snapshot.durationSeconds, handleSeek,
  })
  return {
    status: snapshot.status,
    isActuallyPlaying: snapshot.status === 'playing',
    currentTime: snapshot.currentTimeSeconds,
    duration: snapshot.durationSeconds,
    bufferedRanges: snapshot.bufferedRanges,
    showInitialLoading: snapshot.initialLoading,
    isSeeking: snapshot.seeking ?? false,
    error: snapshot.error,
    ...volume,
    handleSeek, handleSeekStart, handleSeekEnd,
  }
}
