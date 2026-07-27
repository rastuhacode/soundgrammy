import type { AuthStatus, AuthUser } from '@/types'

export const INITIAL_BACKOFF_MS = 2_000
export const MAX_BACKOFF_MS = 30_000
/** Long idle while online; woken early by network-down / stop. */
export const ONLINE_IDLE_MS = 86_400_000

export type ReconnectPhase = 'connecting' | 'online' | 'offline'

export interface BackoffState {
  current: () => number
  /** Double delay after a failed attempt (capped). */
  fail: () => void
  /** Reset to the initial delay (e.g. browser came online). */
  reset: () => void
}

export function createBackoffState(
  initial = INITIAL_BACKOFF_MS,
  max = MAX_BACKOFF_MS,
): BackoffState {
  let delay = initial
  return {
    current: () => delay,
    fail: () => {
      delay = Math.min(delay * 2, max)
    },
    reset: () => {
      delay = initial
    },
  }
}

export function nextBackoffMs(current: number, max = MAX_BACKOFF_MS): number {
  return Math.min(current * 2, max)
}

export interface ReconnectLoopDeps {
  refreshAuth: () => Promise<AuthStatus>
  getPhase: () => ReconnectPhase
  setPhase: (phase: ReconnectPhase) => void
  onUser: (user: AuthUser) => void
  onConnected: () => Promise<void>
  backoff: BackoffState
  /** Resolves after `ms`, or sooner when the returned wake is invoked. */
  sleep: (ms: number) => { promise: Promise<void>, wake: () => void }
  isCancelled: () => boolean
}

/**
 * Telegram-like reconnect loop: retry `refreshAuth` with backoff until online,
 * idle while online, and resume when phase drops offline (caller wakes `sleep`).
 */
export async function runReconnectLoop(deps: ReconnectLoopDeps): Promise<void> {
  while (!deps.isCancelled()) {
    if (deps.getPhase() === 'online') {
      await deps.sleep(ONLINE_IDLE_MS).promise
      continue
    }

    deps.setPhase('connecting')
    try {
      const refreshed = await deps.refreshAuth()
      if (deps.isCancelled()) return
      if (!refreshed.authorized || !refreshed.user) {
        // Session cleared server-side; caller handles auth:revoked.
        return
      }
      deps.onUser(refreshed.user)
      deps.setPhase('online')
      deps.backoff.reset()
      await deps.onConnected()
    }
    catch {
      if (deps.isCancelled()) return
      deps.setPhase('offline')
      const wait = deps.backoff.current()
      deps.backoff.fail()
      await deps.sleep(wait).promise
    }
  }
}

/** Browser-timer sleep with wake, for the React hook. */
export function createTimerSleep(): ReconnectLoopDeps['sleep'] {
  return (ms) => {
    let timer: ReturnType<typeof setTimeout> | null = null
    let wakeFn: (() => void) | null = null
    const promise = new Promise<void>((resolve) => {
      wakeFn = () => {
        if (timer) {
          clearTimeout(timer)
          timer = null
        }
        wakeFn = null
        resolve()
      }
      timer = setTimeout(() => {
        timer = null
        wakeFn = null
        resolve()
      }, ms)
    })
    return {
      promise,
      wake: () => {
        wakeFn?.()
      },
    }
  }
}
