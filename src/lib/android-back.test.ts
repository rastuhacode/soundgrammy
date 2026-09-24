import { describe, expect, it, vi } from 'vitest'
import { AndroidBackStack } from './android-back'

describe('Android Back stack', () => {
  it('closes one layer at a time and restores native Back at the root', async () => {
    let pressBack: (() => void) | undefined
    const unregister = vi.fn(async () => {})
    const listen = vi.fn(async (handler: () => void) => {
      pressBack = handler
      return { unregister }
    })
    const stack = new AndroidBackStack(listen)
    const closePlaylist = vi.fn()
    const closePlayer = vi.fn()
    const closeQueue = vi.fn()

    const removePlaylist = stack.add(closePlaylist)
    await stack.settled()
    const removePlayer = stack.add(closePlayer)
    const removeQueue = stack.add(closeQueue)
    await stack.settled()

    expect(listen).toHaveBeenCalledTimes(1)
    pressBack?.()
    expect(closeQueue).toHaveBeenCalledTimes(1)
    expect(closePlayer).not.toHaveBeenCalled()
    expect(closePlaylist).not.toHaveBeenCalled()

    removeQueue()
    pressBack?.()
    expect(closePlayer).toHaveBeenCalledTimes(1)
    expect(closePlaylist).not.toHaveBeenCalled()

    removePlayer()
    pressBack?.()
    expect(closePlaylist).toHaveBeenCalledTimes(1)

    removePlaylist()
    await stack.settled()
    expect(unregister).toHaveBeenCalledTimes(1)
    pressBack?.()
    expect(closePlaylist).toHaveBeenCalledTimes(1)
  })

  it('unregisters if the last layer closes before native registration finishes', async () => {
    let finishRegistration: ((listener: { unregister: () => Promise<void> }) => void) | undefined
    const unregister = vi.fn(async () => {})
    const stack = new AndroidBackStack(() => new Promise((resolve) => {
      finishRegistration = resolve
    }))

    const remove = stack.add(vi.fn())
    await Promise.resolve()
    remove()
    finishRegistration?.({ unregister })
    await stack.settled()

    expect(unregister).toHaveBeenCalledTimes(1)
  })

  it('keeps one listener when a different layer opens during registration', async () => {
    let finishRegistration: ((listener: { unregister: () => Promise<void> }) => void) | undefined
    let pressBack: (() => void) | undefined
    const unregister = vi.fn(async () => {})
    const listen = vi.fn((handler: () => void) => {
      pressBack = handler
      return new Promise<{ unregister: () => Promise<void> }>((resolve) => {
        finishRegistration = resolve
      })
    })
    const stack = new AndroidBackStack(listen)
    const first = stack.add(vi.fn())
    await Promise.resolve()
    first()
    const secondClose = vi.fn()
    stack.add(secondClose)
    finishRegistration?.({ unregister })
    await stack.settled()

    pressBack?.()
    expect(secondClose).toHaveBeenCalledTimes(1)
    expect(listen).toHaveBeenCalledTimes(1)
    expect(unregister).not.toHaveBeenCalled()
  })

  it('prefers an overlay over a route even if the route registered later', async () => {
    let pressBack: (() => void) | undefined
    const stack = new AndroidBackStack(async (handler) => {
      pressBack = handler
      return { unregister: async () => {} }
    })
    const closeOverlay = vi.fn()
    const closeRoute = vi.fn()

    stack.add(closeOverlay, 1)
    stack.add(closeRoute, 0)
    await stack.settled()
    pressBack?.()

    expect(closeOverlay).toHaveBeenCalledTimes(1)
    expect(closeRoute).not.toHaveBeenCalled()
  })
})
