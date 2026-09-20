import { useCallback, useEffect, useSyncExternalStore } from 'react'
import { useAudioControls } from './use-audio-controls'
import { useAudioVolume } from './use-audio-volume'
import { api, onNativeListenStats } from '@/lib/api'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { useSelectedAudioEngine } from './engine-factory'
import { connectPlayerEngine } from './player-integration'

export function useAudioEngine() {
  const engine = useSelectedAudioEngine()
  const subscribe = useCallback((notify: () => void) => engine.subscribe(notify), [engine])
  const snapshot = useSyncExternalStore(subscribe, engine.getSnapshot.bind(engine))
  useEffect(() => connectPlayerEngine(engine, {
    notifyPlaying: () => {}, notifyActivityStopped: () => {}, notifyCompleted: () => {},
  }), [engine])
  useEffect(() => {
    let active = true
    const subscription = onNativeListenStats((stats) => {
      if (active) useListenStatsStore.getState().upsert(stats)
    })
    const refresh = () => void api.listListenStats().then((stats) => {
      if (active) useListenStatsStore.getState().hydrate(useListenStatsStore.getState().enabled, stats)
    }).catch(() => {})
    refresh()
    window.addEventListener('pageshow', refresh)
    document.addEventListener('visibilitychange', refresh)
    return () => {
      active = false
      void subscription.then(unlisten => unlisten())
      window.removeEventListener('pageshow', refresh)
      document.removeEventListener('visibilitychange', refresh)
    }
  }, [])
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
