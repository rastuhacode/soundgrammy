// @vitest-environment jsdom
import { act, createElement, StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { useProxySettings } from './use-proxy-settings'
import { useSessionStore } from '@/stores/session-store'
import { deferred } from '@/test-support/fixtures'
import type { ProxySettingsView } from '@/types'

vi.mock('@/lib/api', () => ({ api: {
  getProxySettings: vi.fn(), setProxySettings: vi.fn(), parseProxyLink: vi.fn(),
} }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
const view: ProxySettingsView = {
  enabled: false, active: false, server: 'localhost', port: 1443,
  secret: 'proxy-secret', applyError: null, link: null, telegramOnline: false,
}
beforeEach(() => {
  vi.clearAllMocks()
  useSessionStore.getState().clearSession()
  vi.mocked(api.getProxySettings).mockResolvedValue(view)
})

async function mountProxy(enabled = true, strict = false) {
  let controller!: ReturnType<typeof useProxySettings>
  function Probe() {
    controller = useProxySettings(enabled)
    return null
  }
  const root = createRoot(document.createElement('div'))
  await act(async () => root.render(strict
    ? createElement(StrictMode, null, createElement(Probe))
    : createElement(Probe)))
  return { get controller() {
    return controller
  }, root }
}

describe('proxy settings controller', () => {
  it('preserves the apply error while exposing recovered backend status before login', async () => {
    const h = await mountProxy()
    vi.mocked(api.setProxySettings).mockRejectedValueOnce('Reconnect failed')
    vi.mocked(api.getProxySettings).mockResolvedValueOnce({ ...view, enabled: true, applyError: 'Proxy unreachable' })
    try {
      await act(async () => h.controller.apply({ ...view, enabled: true }))
      expect(h.controller.error).toBe('Reconnect failed')
      expect(h.controller.view?.enabled).toBe(true)
      expect(h.controller.view?.applyError).toBe('Proxy unreachable')
      expect(h.controller.busy).toBe(false)
      expect(h.controller.busyLabel).toBeNull()
    }
    finally { await act(async () => h.root.unmount()) }
  })

  it('serializes changes from separate settings surfaces', async () => {
    const first = await mountProxy()
    const second = await mountProxy()
    const pending = deferred<ProxySettingsView>()
    vi.mocked(api.setProxySettings).mockReturnValueOnce(pending.promise).mockResolvedValueOnce(view)
    let firstRequest!: Promise<void>
    let secondRequest!: Promise<void>
    try {
      await act(async () => {
        firstRequest = first.controller.apply({ ...view, enabled: true })
        secondRequest = second.controller.apply(view)
      })
      expect(api.setProxySettings).toHaveBeenCalledTimes(1)
      await act(async () => {
        pending.resolve({ ...view, enabled: true })
        await Promise.all([firstRequest, secondRequest])
      })
      expect(api.setProxySettings).toHaveBeenCalledTimes(2)
      expect(first.controller.view?.enabled).toBe(true)
      expect(second.controller.view?.enabled).toBe(false)
    }
    finally {
      await act(async () => first.root.unmount())
      await act(async () => second.root.unmount())
    }
  })

  it('does not load a second controller when the form shares its parent controller', async () => {
    const h = await mountProxy(false)
    expect(api.getProxySettings).not.toHaveBeenCalled()
    await act(async () => h.root.unmount())
  })

  it('attaches correctly under StrictMode', async () => {
    const h = await mountProxy(true, true)
    expect(h.controller.loaded).toBe(true)
    expect(h.controller.view).toEqual(view)
    await act(async () => h.root.unmount())
  })

  it('discards parse results after logout', async () => {
    const pending = deferred<ProxySettingsView>()
    vi.mocked(api.parseProxyLink).mockReturnValueOnce(pending.promise)
    const h = await mountProxy()
    let request!: Promise<unknown>
    try {
      await act(async () => {
        request = h.controller.parseLink('tg://proxy')
      })
      useSessionStore.getState().clearSession()
      await act(async () => {
        pending.resolve(view)
        expect(await request).toBeNull()
      })
    }
    finally { await act(async () => h.root.unmount()) }
  })
})
