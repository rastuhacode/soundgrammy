import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { useAudioPlaybackState } from './audio-playback-state'
import { useAudioSource } from './use-audio-source'
import { useAudioSeek } from './use-audio-seek'
import { canSyncMediaPlaybackState, useMseColdStartPrime } from './use-mse-cold-start-prime'
import type { HtmlDriver, HtmlSession } from './html-driver'
import type { AudioEngineError, AudioEngineStatus } from '../engine'

export function HtmlEngineHost({ driver }: { driver: HtmlDriver }) {
  const current = useSyncExternalStore(driver.subscribe, driver.getSnapshot)
  return current ? <HtmlAttempt key={current.key} session={current.session} /> : null
}

/** One element per source generation prevents queued DOM events crossing attempts. */
function HtmlAttempt({ session }: { session: HtmlSession }) {
  const { request, observer } = session
  const [currentTime, setCurrentTime] = useState(0)
  const [duration, setDuration] = useState(request.expectedDurationSeconds ?? 0)
  const volumeRef = useRef(100)
  const seekToEndRef = useRef(false)
  const activeRef = useRef(true)
  const statusRef = useRef<AudioEngineStatus>('loading')
  const track = { id: request.trackId, duration: request.expectedDurationSeconds ?? null }

  const reportStatus = useCallback((status: AudioEngineStatus) => {
    if (!activeRef.current || statusRef.current === status) return
    statusRef.current = status
    observer.state({ status })
  }, [observer])
  const failed = useCallback((code: AudioEngineError['code'] = 'source-unavailable') => {
    if (!activeRef.current) return
    observer.failed({ code, message: 'Audio playback failed.', recoverable: true })
  }, [observer])

  // The callbacks run after these refs exist, including the first source attach.
  const setPlaying = useCallback((playing: boolean) => {
    if (!activeRef.current) return
    if (!playing) failed()
    else reportStatus('playing')
  }, [failed, reportStatus])
  const playback = useAudioPlaybackState({ isPlaying: false, setPlaying })
  const {
    audioRef, isPlayingRef, loadGenerationRef, loadedTrackIdRef, sourceErrorRef,
    pendingSeekRef, resumeAfterSeekRef, isSeekingRef, playAudio, resetSeekRefs,
  } = playback
  const applyVolume = () => {
    if (audioRef.current) audioRef.current.volume = volumeRef.current / 100
  }
  const source = useAudioSource({
    ...playback, track, applyVolume, setCurrentTime, setDuration, setPlaying,
  })
  const seek = useAudioSeek({
    ...playback, ...source, duration, currentTime, setCurrentTime, trackId: track.id,
  })
  const msePrimeRef = useMseColdStartPrime({
    ...playback, trackId: track.id, setCurrentTime,
  })

  const controlsRef = useRef({ source, seek, playAudio, applyVolume, duration })
  useEffect(() => {
    controlsRef.current = { source, seek, playAudio, applyVolume, duration }
  })

  useEffect(() => {
    activeRef.current = true
    let detach: (() => void) | undefined
    let disposed = false
    // Source initialization resets seek refs in a microtask; apply queued commands after it.
    queueMicrotask(() => {
      if (disposed) return
      detach = session.attach({
        play: () => {
          isPlayingRef.current = true
          const audio = audioRef.current
          if (!audio?.src || loadedTrackIdRef.current !== track.id) return
          if (pendingSeekRef.current !== null) {
            resumeAfterSeekRef.current = true
            controlsRef.current.seek.finishPendingSeek(audio)
          }
          else controlsRef.current.playAudio(audio, loadGenerationRef.current)
        },
        pause: () => {
          isPlayingRef.current = false
          resumeAfterSeekRef.current = false
          audioRef.current?.pause()
          reportStatus('paused')
        },
        seek: (seconds) => {
          const duration = controlsRef.current.duration
          seekToEndRef.current = duration > 0 && seconds >= duration
          controlsRef.current.seek.handleSeek(seconds)
        },
        beginSeek: () => controlsRef.current.seek.handleSeekStart(),
        endSeek: () => controlsRef.current.seek.handleSeekEnd(),
        volume: (percent) => {
          volumeRef.current = percent
          controlsRef.current.applyVolume()
        },
        dispose: () => {
          activeRef.current = false
          isPlayingRef.current = false
          resetSeekRefs()
          controlsRef.current.source.disposeSource()
        },
      })
    })
    return () => {
      disposed = true
      activeRef.current = false
      detach?.()
    }
    // The session is immutable; handlers read the current hook functions through controlsRef.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session])

  useEffect(() => {
    if (!activeRef.current) return
    observer.state({
      currentTimeSeconds: currentTime, durationSeconds: duration,
      bufferedRanges: seek.bufferedRanges, initialLoading: source.showInitialLoading,
    })
  }, [currentTime, duration, observer, seek.bufferedRanges, source.showInitialLoading])

  const stable = () => activeRef.current && canSyncMediaPlaybackState({
    trackId: track.id, loadedTrackId: loadedTrackIdRef.current,
    pendingSeek: pendingSeekRef.current, isSeeking: isSeekingRef.current,
    sourceFailed: sourceErrorRef.current,
  })
  const loaded = () => activeRef.current && loadedTrackIdRef.current === track.id

  return (
    <audio
      ref={audioRef}
      className="hidden"
      preload="metadata"
      onLoadedMetadata={(event) => {
        if (!loaded()) return
        seek.onLoadedMetadata(event)
        if (Number.isFinite(event.currentTarget.duration)) setDuration(event.currentTarget.duration)
        if (statusRef.current === 'loading') reportStatus('ready')
      }}
      onCanPlay={(event) => {
        if (!loaded()) return
        seek.onCanPlay(event)
        if (event.currentTarget.paused && statusRef.current === 'loading') reportStatus('ready')
      }}
      onSeeked={(event) => {
        if (!loaded()) return
        seek.onSeeked(event)
        if (!event.currentTarget.seeking && pendingSeekRef.current === null && !isSeekingRef.current) {
          if (!event.currentTarget.paused) reportStatus('playing')
          else if (!isPlayingRef.current) reportStatus('paused')
        }
      }}
      onTimeUpdate={(event) => {
        if (!loaded() || isSeekingRef.current || pendingSeekRef.current !== null || msePrimeRef.current === 'priming') return
        setCurrentTime(event.currentTarget.currentTime)
      }}
      onDurationChange={(event) => {
        if (!loaded()) return
        const value = event.currentTarget.duration
        if (Number.isFinite(value) && value > 0) setDuration(value)
      }}
      onPlaying={(event) => {
        if (!loaded()) return
        if (!isPlayingRef.current) {
          event.currentTarget.pause()
          return
        }
        reportStatus('playing')
      }}
      onWaiting={() => {
        if (loaded()) reportStatus('buffering')
      }}
      onStalled={() => {
        if (loaded()) reportStatus('buffering')
      }}
      onSeeking={() => {
        if (loaded()) reportStatus('buffering')
      }}
      onPause={(event) => {
        if (stable() && event.currentTarget.paused && !event.currentTarget.ended) {
          isPlayingRef.current = false
          reportStatus('paused')
        }
      }}
      onEnded={(event) => {
        if (!stable() || !event.currentTarget.ended) return
        if (seekToEndRef.current) {
          reportStatus('paused')
          return
        }
        const metaDuration = track.duration ?? 0
        const actualDuration = event.currentTarget.duration
        if (metaDuration > 5 && actualDuration > 0 && actualDuration < Math.min(2, metaDuration * 0.05)) {
          failed()
          return
        }
        observer.ended()
      }}
      onError={() => {
        if (loaded()) failed()
      }}
    />
  )
}
