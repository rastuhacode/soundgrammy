import type { ProxySettingsView } from '@/types'

export type ProxyDisplayKind = 'checking' | 'on' | 'off' | 'pending' | 'issue'

export interface ProxyDisplayState {
  kind: ProxyDisplayKind
  label: string
  description: string
}

export function getProxyDisplayState(
  view: ProxySettingsView | null,
): ProxyDisplayState {
  if (!view) {
    return {
      kind: 'checking',
      label: 'Proxy: Checking…',
      description: 'Checking the current Telegram proxy status.',
    }
  }

  if (!view.enabled) {
    return {
      kind: 'off',
      label: 'Proxy: Off',
      description: view.telegramOnline
        ? 'Telegram is connected directly. Enable the proxy here if needed.'
        : 'Telegram is not connected. Enable an MTProto proxy here, or connect through a VPN.',
    }
  }

  if (view.applyError) {
    return {
      kind: 'issue',
      label: 'Proxy: Issue',
      description: 'The proxy is enabled but Telegram could not connect through it. Check the settings and apply again.',
    }
  }

  if (view.active) {
    return {
      kind: 'on',
      label: 'Proxy: On',
      description: 'The MTProto proxy is active for Telegram traffic.',
    }
  }

  return {
    kind: 'pending',
    label: 'Proxy: Not active',
    description: 'The proxy is enabled but not active. Apply the settings to reconnect.',
  }
}
