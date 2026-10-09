import { describe, expect, it, vi } from 'vitest'
import { ownEventListeners } from './events'
import { deferred } from '@/test-support/fixtures'
vi.mock('@/lib/app-logger', () => ({ appLogger: { error: vi.fn() } }))
describe('async subscription ownership', () => {
  it('cleans successful listeners even when another registration fails', async () => {
    const unsubscribe = vi.fn()
    const late = deferred<() => void>()
    const cleanup = ownEventListeners([Promise.resolve(unsubscribe), Promise.reject('failed'), late.promise])
    await Promise.resolve()
    cleanup()
    expect(unsubscribe).toHaveBeenCalledTimes(1)
    const lateUnsubscribe = vi.fn()
    late.resolve(lateUnsubscribe)
    await Promise.resolve()
    expect(lateUnsubscribe).toHaveBeenCalledTimes(1)
  })
})
