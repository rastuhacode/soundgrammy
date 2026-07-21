import type { ListenEndReason } from '@/types'

/** Pending abandonment reason set by player-store before changing tracks. */
let pendingEndReason: ListenEndReason | null = null

export function setPendingListenEndReason(reason: ListenEndReason) {
  pendingEndReason = reason
}

export function clearPendingListenEndReason() {
  pendingEndReason = null
}

/** Takes and clears the pending reason; defaults to `replaced` when unset. */
export function takePendingListenEndReason(
  fallback: ListenEndReason = 'replaced',
): ListenEndReason {
  const reason = pendingEndReason ?? fallback
  pendingEndReason = null
  return reason
}

/** Effective listened ms: min(L, D); completed uses D when known. */
export function effectiveListenedMs(
  listenedMs: number,
  durationMs: number | null | undefined,
  endReason: ListenEndReason,
): number {
  const listened = Math.max(0, listenedMs)
  if (endReason === 'completed' && durationMs != null && durationMs > 0) {
    return durationMs
  }
  if (durationMs != null && durationMs > 0) {
    return Math.min(listened, durationMs)
  }
  return listened
}

/** Track duration from Telegram metadata (seconds) → ms. */
export function trackDurationMs(durationSeconds: number | null | undefined): number | null {
  if (durationSeconds == null || durationSeconds <= 0) return null
  return Math.round(durationSeconds * 1000)
}

export interface ListenAttemptClock {
  listenedMs: number
  playingSince: number | null
}

export function createAttemptClock(): ListenAttemptClock {
  return { listenedMs: 0, playingSince: null }
}

export function clockOnPlay(clock: ListenAttemptClock, now: number): ListenAttemptClock {
  if (clock.playingSince != null) return clock
  return { ...clock, playingSince: now }
}

export function clockOnPause(clock: ListenAttemptClock, now: number): ListenAttemptClock {
  if (clock.playingSince == null) return clock
  return {
    listenedMs: clock.listenedMs + (now - clock.playingSince),
    playingSince: null,
  }
}

export function clockListenedMs(clock: ListenAttemptClock, now: number): number {
  if (clock.playingSince == null) return clock.listenedMs
  return clock.listenedMs + (now - clock.playingSince)
}
