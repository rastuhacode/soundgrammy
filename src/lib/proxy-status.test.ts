import { describe, expect, it } from 'vitest'
import { getProxyDisplayState } from './proxy-status'
import type { ProxySettingsView } from '@/types'

function proxyView(
  overrides: Partial<ProxySettingsView> = {},
): ProxySettingsView {
  return {
    enabled: false,
    server: '',
    port: 1443,
    secret: '',
    active: false,
    applyError: null,
    link: null,
    telegramOnline: true,
    ...overrides,
  }
}

describe('getProxyDisplayState', () => {
  it('reports status loading before settings arrive', () => {
    expect(getProxyDisplayState(null).kind).toBe('checking')
  })

  it('reports a disabled proxy as off', () => {
    expect(getProxyDisplayState(proxyView())).toMatchObject({
      kind: 'off',
      label: 'Proxy: Off',
    })
  })

  it('reports on only when the enabled proxy is active', () => {
    expect(getProxyDisplayState(proxyView({ enabled: true, active: true }))).toMatchObject({
      kind: 'on',
      label: 'Proxy: On',
    })
    expect(getProxyDisplayState(proxyView({ enabled: true }))).toMatchObject({
      kind: 'pending',
      label: 'Proxy: Not active',
    })
  })

  it('gives an enabled proxy connection error priority', () => {
    expect(getProxyDisplayState(proxyView({
      enabled: true,
      active: true,
      applyError: 'connection failed',
    }))).toMatchObject({
      kind: 'issue',
      label: 'Proxy: Issue',
    })
  })
})
