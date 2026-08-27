import { describe, expect, it, vi } from 'vitest'
import { createWakeLockScheduler } from './fullscreen-wake-lock'

describe('fullscreen wake lock scheduler', () => {
  it('applies rapid transitions in request order', async () => {
    const calls: boolean[] = []
    let finishFirst: (() => void) | undefined
    const setNative = vi.fn(async (enabled: boolean) => {
      calls.push(enabled)
      if (calls.length === 1) {
        await new Promise<void>((resolve) => {
          finishFirst = resolve
        })
      }
    })
    const schedule = createWakeLockScheduler(setNative)

    const enabling = schedule(true)
    const disabling = schedule(false)
    await Promise.resolve()
    expect(calls).toEqual([true])

    finishFirst?.()
    await Promise.all([enabling, disabling])
    expect(calls).toEqual([true, false])
  })

  it('continues after a native update fails', async () => {
    const setNative = vi.fn()
      .mockRejectedValueOnce(new Error('unsupported'))
      .mockResolvedValueOnce(undefined)
    const schedule = createWakeLockScheduler(setNative)

    await schedule(true)
    await schedule(false)

    expect(setNative).toHaveBeenNthCalledWith(1, true)
    expect(setNative).toHaveBeenNthCalledWith(2, false)
  })
})
