import { describe, expect, it, vi } from 'vitest'
import type { AuthUser } from '@/types'
import {
  createBackoffState,
  INITIAL_BACKOFF_MS,
  MAX_BACKOFF_MS,
  nextBackoffMs,
  ONLINE_IDLE_MS,
  runReconnectLoop,
  type ReconnectPhase,
} from './telegram-reconnect'

const user: AuthUser = {
  id: 1,
  firstName: 'Ada',
  lastName: null,
  username: 'ada',
  phone: null,
}

function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>((r) => {
    resolve = r
  })
  return { promise, resolve }
}

function controllableSleep() {
  const pending: Array<{ resolve: () => void }> = []
  const sleep = (_ms: number) => {
    void _ms
    const d = deferred()
    pending.push(d)
    return {
      promise: d.promise,
      wake: () => d.resolve(),
    }
  }
  return {
    sleep,
    wakeAll: () => {
      while (pending.length > 0) pending.shift()!.resolve()
    },
    wakeOne: () => {
      pending.shift()?.resolve()
    },
    pendingCount: () => pending.length,
  }
}

describe('nextBackoffMs', () => {
  it('doubles until the cap', () => {
    expect(nextBackoffMs(2_000)).toBe(4_000)
    expect(nextBackoffMs(16_000)).toBe(30_000)
    expect(nextBackoffMs(30_000)).toBe(30_000)
  })
})

describe('createBackoffState', () => {
  it('fails with exponential backoff and resets', () => {
    const backoff = createBackoffState()
    expect(backoff.current()).toBe(INITIAL_BACKOFF_MS)
    backoff.fail()
    expect(backoff.current()).toBe(4_000)
    backoff.fail()
    expect(backoff.current()).toBe(8_000)
    for (let i = 0; i < 10; i++) backoff.fail()
    expect(backoff.current()).toBe(MAX_BACKOFF_MS)
    backoff.reset()
    expect(backoff.current()).toBe(INITIAL_BACKOFF_MS)
  })
})

describe('runReconnectLoop', () => {
  it('retries after unreachable then succeeds', async () => {
    let phase: ReconnectPhase = 'connecting'
    const phases: ReconnectPhase[] = []
    const sleepCtrl = controllableSleep()
    const onUser = vi.fn()
    const onConnected = vi.fn(async () => {})
    let attempts = 0
    let cancelled = false

    const loop = runReconnectLoop({
      refreshAuth: async () => {
        attempts += 1
        if (attempts < 3) throw new Error('telegram unreachable')
        return { authorized: true, user }
      },
      getPhase: () => phase,
      setPhase: (next) => {
        phase = next
        phases.push(next)
      },
      onUser,
      onConnected,
      backoff: createBackoffState(),
      sleep: sleepCtrl.sleep,
      isCancelled: () => cancelled,
    })

    // Fail #1 → offline sleep
    await vi.waitFor(() => expect(sleepCtrl.pendingCount()).toBe(1))
    expect(phase).toBe('offline')
    sleepCtrl.wakeOne()

    // Fail #2 → offline sleep
    await vi.waitFor(() => expect(sleepCtrl.pendingCount()).toBe(1))
    expect(phase).toBe('offline')
    sleepCtrl.wakeOne()

    // Success → online idle sleep
    await vi.waitFor(() => expect(phase).toBe('online'))
    expect(onUser).toHaveBeenCalledWith(user)
    expect(onConnected).toHaveBeenCalledOnce()
    expect(phases.filter(p => p === 'connecting').length).toBeGreaterThanOrEqual(3)

    cancelled = true
    sleepCtrl.wakeAll()
    await loop
  })

  it('stops when refresh returns unauthorized', async () => {
    let phase: ReconnectPhase = 'connecting'
    const onUser = vi.fn()
    const onConnected = vi.fn(async () => {})

    await runReconnectLoop({
      refreshAuth: async () => ({ authorized: false, user: null }),
      getPhase: () => phase,
      setPhase: (next) => {
        phase = next
      },
      onUser,
      onConnected,
      backoff: createBackoffState(),
      sleep: () => {
        const d = deferred()
        return { promise: d.promise, wake: d.resolve }
      },
      isCancelled: () => false,
    })

    expect(onUser).not.toHaveBeenCalled()
    expect(onConnected).not.toHaveBeenCalled()
  })

  it('idles while online and resumes after phase drops offline', async () => {
    let phase: ReconnectPhase = 'online'
    const sleepCtrl = controllableSleep()
    const onConnected = vi.fn(async () => {})
    let cancelled = false
    let refreshCount = 0

    const loop = runReconnectLoop({
      refreshAuth: async () => {
        refreshCount += 1
        return { authorized: true, user }
      },
      getPhase: () => phase,
      setPhase: (next) => {
        phase = next
      },
      onUser: () => {},
      onConnected,
      backoff: createBackoffState(),
      sleep: (ms) => {
        expect(ms === ONLINE_IDLE_MS || ms >= INITIAL_BACKOFF_MS).toBe(true)
        return sleepCtrl.sleep(ms)
      },
      isCancelled: () => cancelled,
    })

    await vi.waitFor(() => expect(sleepCtrl.pendingCount()).toBe(1))
    expect(refreshCount).toBe(0)

    phase = 'offline'
    sleepCtrl.wakeOne()

    await vi.waitFor(() => expect(onConnected).toHaveBeenCalledOnce())
    expect(refreshCount).toBe(1)
    expect(phase).toBe('online')

    cancelled = true
    sleepCtrl.wakeAll()
    await loop
  })

  it('exits promptly when cancelled during backoff', async () => {
    let phase: ReconnectPhase = 'connecting'
    let cancelled = false
    const sleepCtrl = controllableSleep()

    const loop = runReconnectLoop({
      refreshAuth: async () => {
        throw new Error('telegram unreachable')
      },
      getPhase: () => phase,
      setPhase: (next) => {
        phase = next
      },
      onUser: () => {},
      onConnected: async () => {},
      backoff: createBackoffState(),
      sleep: sleepCtrl.sleep,
      isCancelled: () => cancelled,
    })

    await vi.waitFor(() => expect(phase).toBe('offline'))
    cancelled = true
    sleepCtrl.wakeOne()
    await loop
  })
})
