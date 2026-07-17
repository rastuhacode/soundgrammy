import { useEffect, useRef } from 'react'
import { api } from '@/lib/api'
import {
  clockListenedMs,
  clockOnPause,
  clockOnPlay,
  clearPendingListenEndReason,
  createAttemptClock,
  takePendingListenEndReason,
  trackDurationMs,
  type ListenAttemptClock,
} from '@/lib/listen-tracker'
import { usePlayerStore } from '@/stores/player-store'
import type { ListenEndReason } from '@/types'

/**
 * Observes player track / play state and records listen attempts.
 * Call `notifyCompleted` from the audio `ended` handler (before next/repeat).
 */
export function useListenTracker(options: {
  trackId: number | null
  durationSeconds: number | null | undefined
  isPlaying: boolean
}): { notifyCompleted: (restartSameTrack?: boolean) => void } {
  const { trackId, durationSeconds, isPlaying } = options
  const listenAttemptEpoch = usePlayerStore(state => state.listenAttemptEpoch)

  const attemptTrackIdRef = useRef<number | null>(null)
  const attemptDurationSecRef = useRef<number | null | undefined>(null)
  const clockRef = useRef<ListenAttemptClock>(createAttemptClock())
  const isPlayingRef = useRef(isPlaying)
  const trackIdRef = useRef(trackId)
  const listenAttemptEpochRef = useRef(listenAttemptEpoch)

  useEffect(() => {
    isPlayingRef.current = isPlaying
  }, [isPlaying])

  useEffect(() => {
    trackIdRef.current = trackId
  }, [trackId])

  const persistEnd = (
    id: number,
    endReason: ListenEndReason,
    listenedMs: number,
    durationSec: number | null | undefined,
  ) => {
    const durationMs = trackDurationMs(durationSec)
    void api.recordListenEnd({
      trackId: id,
      listenedMs,
      durationMs,
      endReason,
    }).catch(() => {
      // Best-effort; do not interrupt playback.
    })
  }

  /** Clears local attempt state immediately; returns snapshot for async persist. */
  const closeAttemptLocally = (): {
    id: number
    listenedMs: number
    durationSec: number | null | undefined
  } | null => {
    const id = attemptTrackIdRef.current
    if (id == null) return null
    const now = performance.now()
    const listenedMs = Math.round(clockListenedMs(clockRef.current, now))
    const durationSec = attemptDurationSecRef.current
    attemptTrackIdRef.current = null
    attemptDurationSecRef.current = null
    clockRef.current = createAttemptClock()
    return { id, listenedMs, durationSec }
  }

  const startAttempt = (id: number, durationSec: number | null | undefined) => {
    clearPendingListenEndReason()
    attemptTrackIdRef.current = id
    attemptDurationSecRef.current = durationSec
    clockRef.current = createAttemptClock()
    if (isPlayingRef.current) {
      clockRef.current = clockOnPlay(clockRef.current, performance.now())
    }
    void api.recordListenStart(id).catch(() => {
      // Best-effort; do not interrupt playback.
    })
  }

  // Track identity changes: end previous attempt, start new when track present.
  useEffect(() => {
    const activeId = attemptTrackIdRef.current

    if (activeId != null && activeId !== trackId) {
      const closed = closeAttemptLocally()
      if (closed) {
        const reason = takePendingListenEndReason('replaced')
        persistEnd(closed.id, reason, closed.listenedMs, closed.durationSec)
      }
    }

    if (trackId != null && attemptTrackIdRef.current !== trackId) {
      startAttempt(trackId, durationSeconds)
    }

    if (trackId == null && attemptTrackIdRef.current != null) {
      const closed = closeAttemptLocally()
      if (closed) {
        const reason = takePendingListenEndReason('stopped')
        persistEnd(closed.id, reason, closed.listenedMs, closed.durationSec)
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [trackId])

  // Same track id, new queue row (duplicate entries / single-track next|prev).
  useEffect(() => {
    if (listenAttemptEpochRef.current === listenAttemptEpoch) return
    listenAttemptEpochRef.current = listenAttemptEpoch

    if (trackId == null || attemptTrackIdRef.current !== trackId) return

    const closed = closeAttemptLocally()
    if (closed) {
      const reason = takePendingListenEndReason('skipped')
      persistEnd(closed.id, reason, closed.listenedMs, closed.durationSec)
    }
    startAttempt(trackId, durationSeconds)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [listenAttemptEpoch])

  // Keep duration metadata fresh for the active attempt.
  useEffect(() => {
    if (attemptTrackIdRef.current != null && trackId === attemptTrackIdRef.current) {
      attemptDurationSecRef.current = durationSeconds
    }
  }, [durationSeconds, trackId])

  // Pause / resume: accumulate wall-clock only while playing.
  // Also reopen an attempt when play resumes after a natural end left the
  // same track loaded (trackId does not change, so the track effect is silent).
  useEffect(() => {
    if (trackId != null && isPlaying && attemptTrackIdRef.current == null) {
      startAttempt(trackId, durationSeconds)
      return
    }
    if (attemptTrackIdRef.current == null) return
    const now = performance.now()
    clockRef.current = isPlaying
      ? clockOnPlay(clockRef.current, now)
      : clockOnPause(clockRef.current, now)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying])

  // App quit / tab close → interrupted.
  useEffect(() => {
    const onPageHide = () => {
      const closed = closeAttemptLocally()
      if (!closed) return
      persistEnd(closed.id, 'interrupted', closed.listenedMs, closed.durationSec)
    }
    window.addEventListener('pagehide', onPageHide)
    window.addEventListener('beforeunload', onPageHide)
    return () => {
      window.removeEventListener('pagehide', onPageHide)
      window.removeEventListener('beforeunload', onPageHide)
    }
  }, [])

  const notifyCompleted = (restartSameTrack = false) => {
    const closed = closeAttemptLocally()
    if (!closed) return
    persistEnd(closed.id, 'completed', closed.listenedMs, closed.durationSec)
    // Same track stays active without a trackId change — open a new attempt
    // (repeat-one, or repeat-all wrapping onto the same track).
    if (restartSameTrack && trackIdRef.current === closed.id) {
      startAttempt(closed.id, closed.durationSec)
    }
  }

  return { notifyCompleted }
}
