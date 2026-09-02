import { useEffect, useRef } from 'react'
import { api } from '@/lib/api'
import {
  clockListenedMs,
  clockOnPause,
  clockOnPlay,
  clearPendingListenEndReason,
  createAttemptClock,
  lastFmThresholdMs,
  monotonicNow,
  takePendingListenEndReason,
  trackDurationMs,
  type ListenAttemptClock,
} from '@/lib/listen-tracker'
import { useLastFmStore } from '@/stores/lastfm-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { usePlayerStore } from '@/stores/player-store'
import type { ListenEndReason } from '@/types'

interface PlaybackAttempt {
  attemptId: string
  trackId: number
  durationSec: number | null | undefined
  clock: ListenAttemptClock
  localActive: boolean
  localBaselineMs: number
  lastFmActive: boolean
  lastFmBaselineMs: number
  lastFmQualifiedSent: boolean
  lastFmQualification: Promise<void> | null
}

function newAttemptId(): string {
  return globalThis.crypto?.randomUUID?.()
    ?? `attempt-${Date.now()}-${Math.random().toString(36).slice(2)}`
}

/**
 * Owns one neutral playback clock and independent local-statistics/Last.fm sinks.
 * Media event callbacks, not requested player state, determine played wall time.
 */
export function useListenTracker(options: {
  trackId: number | null
  durationSeconds: number | null | undefined
}): {
  notifyPlaying: () => void
  notifyActivityStopped: () => void
  notifyCompleted: (restartSameTrack?: boolean) => void
} {
  const { trackId, durationSeconds } = options
  const listenAttemptEpoch = usePlayerStore(state => state.listenAttemptEpoch)
  const statisticsEnabled = useListenStatsStore(state => state.enabled)
  const statisticsClearEpoch = useListenStatsStore(state => state.clearEpoch)
  const lastFmStatus = useLastFmStore(state => state.status)
  const lastFmReady = lastFmStatus?.state === 'connected' && lastFmStatus.enabled

  const attemptRef = useRef<PlaybackAttempt | null>(null)
  const actuallyPlayingRef = useRef(false)
  const qualificationTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const trackIdRef = useRef(trackId)
  const listenAttemptEpochRef = useRef(listenAttemptEpoch)
  const statisticsClearEpochRef = useRef(statisticsClearEpoch)
  const statisticsEnabledRef = useRef(statisticsEnabled)
  const lastFmReadyRef = useRef(lastFmReady)

  useEffect(() => {
    trackIdRef.current = trackId
  }, [trackId])

  useEffect(() => {
    statisticsEnabledRef.current = statisticsEnabled
  }, [statisticsEnabled])

  useEffect(() => {
    lastFmReadyRef.current = lastFmReady
  }, [lastFmReady])

  const clearQualificationTimer = () => {
    if (qualificationTimerRef.current !== null) {
      clearTimeout(qualificationTimerRef.current)
      qualificationTimerRef.current = null
    }
  }

  const lastFmListenedMs = (attempt: PlaybackAttempt, now: number) =>
    Math.max(0, clockListenedMs(attempt.clock, now) - attempt.lastFmBaselineMs)

  const maybeQualifyLastFm = (now: number) => {
    const attempt = attemptRef.current
    if (
      !attempt
      || !attempt.lastFmActive
      || attempt.lastFmQualifiedSent
      || !lastFmReadyRef.current
    ) return
    const threshold = lastFmThresholdMs(attempt.durationSec)
    if (threshold == null) return
    const listenedMs = Math.round(lastFmListenedMs(attempt, now))
    if (listenedMs < threshold) return
    attempt.lastFmQualifiedSent = true
    attempt.lastFmQualification = api
      .lastFmAttemptQualified(attempt.attemptId, listenedMs)
      .catch(() => {})
  }

  const scheduleQualification = () => {
    clearQualificationTimer()
    const attempt = attemptRef.current
    if (
      !attempt
      || !attempt.lastFmActive
      || attempt.lastFmQualifiedSent
      || !actuallyPlayingRef.current
      || !lastFmReadyRef.current
    ) return
    const threshold = lastFmThresholdMs(attempt.durationSec)
    if (threshold == null) return
    const remaining = threshold - lastFmListenedMs(attempt, monotonicNow())
    if (remaining <= 0) {
      maybeQualifyLastFm(monotonicNow())
      return
    }
    qualificationTimerRef.current = setTimeout(() => {
      qualificationTimerRef.current = null
      maybeQualifyLastFm(monotonicNow())
      if (!attemptRef.current?.lastFmQualifiedSent) scheduleQualification()
    }, Math.max(1, Math.ceil(remaining)))
  }

  const activateLastFm = () => {
    const attempt = attemptRef.current
    if (
      !attempt
      || attempt.lastFmActive
      || !actuallyPlayingRef.current
      || !lastFmReadyRef.current
    ) return
    attempt.lastFmActive = true
    attempt.lastFmBaselineMs = clockListenedMs(attempt.clock, monotonicNow())
    attempt.lastFmQualifiedSent = false
    attempt.lastFmQualification = null
    api.lastFmAttemptStarted(attempt.attemptId, attempt.trackId).catch(() => {})
    scheduleQualification()
  }

  const deactivateLastFm = () => {
    const attempt = attemptRef.current
    clearQualificationTimer()
    if (!attempt?.lastFmActive) return
    attempt.lastFmActive = false
    attempt.lastFmQualifiedSent = false
    const pending = attempt.lastFmQualification ?? Promise.resolve()
    pending.finally(() => api.lastFmAttemptEnded(attempt.attemptId).catch(() => {}))
    attempt.lastFmQualification = null
  }

  const persistLocalEnd = (
    attempt: PlaybackAttempt,
    endReason: ListenEndReason,
    totalMs: number,
  ) => {
    if (!attempt.localActive) return
    const listenedMs = Math.max(0, Math.round(totalMs - attempt.localBaselineMs))
    api.recordListenEnd({
      trackId: attempt.trackId,
      listenedMs,
      durationMs: trackDurationMs(attempt.durationSec),
      endReason,
    }).then((result) => {
      if (result) useListenStatsStore.getState().upsert(result.stats)
    }).catch(() => {})
  }

  const finishLastFm = (attempt: PlaybackAttempt, totalMs: number) => {
    clearQualificationTimer()
    if (!attempt.lastFmActive) return
    const threshold = lastFmThresholdMs(attempt.durationSec)
    const listenedMs = Math.max(0, Math.round(totalMs - attempt.lastFmBaselineMs))
    let pending = attempt.lastFmQualification ?? Promise.resolve()
    if (
      !attempt.lastFmQualifiedSent
      && lastFmReadyRef.current
      && threshold != null
      && listenedMs >= threshold
    ) {
      pending = api.lastFmAttemptQualified(attempt.attemptId, listenedMs).catch(() => {})
    }
    pending.finally(() => api.lastFmAttemptEnded(attempt.attemptId).catch(() => {}))
  }

  const closeAttempt = (endReason: ListenEndReason) => {
    const attempt = attemptRef.current
    if (!attempt) return
    const now = monotonicNow()
    attempt.clock = clockOnPause(attempt.clock, now)
    const totalMs = clockListenedMs(attempt.clock, now)
    attemptRef.current = null
    actuallyPlayingRef.current = false
    persistLocalEnd(attempt, endReason, totalMs)
    finishLastFm(attempt, totalMs)
  }

  const startAttempt = (id: number, durationSec: number | null | undefined) => {
    clearPendingListenEndReason()
    const localActive = statisticsEnabledRef.current
    attemptRef.current = {
      attemptId: newAttemptId(),
      trackId: id,
      durationSec,
      clock: createAttemptClock(),
      localActive,
      localBaselineMs: 0,
      lastFmActive: false,
      lastFmBaselineMs: 0,
      lastFmQualifiedSent: false,
      lastFmQualification: null,
    }
    actuallyPlayingRef.current = false
    if (localActive) api.recordListenStart(id).catch(() => {})
  }

  useEffect(() => {
    const attempt = attemptRef.current
    if (!attempt) return
    if (!statisticsEnabled) {
      attempt.localActive = false
      return
    }
    if (!attempt.localActive) {
      attempt.localActive = true
      attempt.localBaselineMs = clockListenedMs(attempt.clock, monotonicNow())
      api.recordListenStart(attempt.trackId).catch(() => {})
    }
  }, [statisticsEnabled])

  useEffect(() => {
    if (statisticsClearEpochRef.current === statisticsClearEpoch) return
    statisticsClearEpochRef.current = statisticsClearEpoch
    const attempt = attemptRef.current
    if (!attempt || !statisticsEnabled) return
    attempt.localActive = true
    attempt.localBaselineMs = clockListenedMs(attempt.clock, monotonicNow())
    api.recordListenStart(attempt.trackId).catch(() => {})
  }, [statisticsClearEpoch, statisticsEnabled])

  useEffect(() => {
    const attempt = attemptRef.current
    if (attempt && attempt.trackId !== trackId) {
      closeAttempt(takePendingListenEndReason(trackId == null ? 'stopped' : 'replaced'))
    }
    if (trackId != null && attemptRef.current?.trackId !== trackId) {
      startAttempt(trackId, durationSeconds)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [trackId])

  useEffect(() => {
    if (listenAttemptEpochRef.current === listenAttemptEpoch) return
    listenAttemptEpochRef.current = listenAttemptEpoch
    if (trackId == null || attemptRef.current?.trackId !== trackId) return
    closeAttempt(takePendingListenEndReason('skipped'))
    startAttempt(trackId, durationSeconds)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [listenAttemptEpoch])

  useEffect(() => {
    const attempt = attemptRef.current
    if (!attempt || attempt.trackId !== trackId) return
    attempt.durationSec = durationSeconds
    scheduleQualification()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [durationSeconds, trackId])

  useEffect(() => {
    if (lastFmReady) activateLastFm()
    else deactivateLastFm()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lastFmReady])

  useEffect(() => {
    const onPageHide = () => closeAttempt('interrupted')
    window.addEventListener('pagehide', onPageHide)
    window.addEventListener('beforeunload', onPageHide)
    return () => {
      window.removeEventListener('pagehide', onPageHide)
      window.removeEventListener('beforeunload', onPageHide)
      clearQualificationTimer()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const notifyPlaying = () => {
    const attempt = attemptRef.current
    if (!attempt) return
    actuallyPlayingRef.current = true
    attempt.clock = clockOnPlay(attempt.clock, monotonicNow())
    activateLastFm()
    scheduleQualification()
  }

  const notifyActivityStopped = () => {
    const attempt = attemptRef.current
    if (!attempt) return
    const now = monotonicNow()
    attempt.clock = clockOnPause(attempt.clock, now)
    actuallyPlayingRef.current = false
    maybeQualifyLastFm(now)
    clearQualificationTimer()
  }

  const notifyCompleted = (restartSameTrack = false) => {
    notifyActivityStopped()
    const completedTrackId = attemptRef.current?.trackId ?? null
    closeAttempt('completed')
    if (restartSameTrack && completedTrackId != null && trackIdRef.current === completedTrackId) {
      startAttempt(completedTrackId, durationSeconds)
    }
  }

  return { notifyPlaying, notifyActivityStopped, notifyCompleted }
}
