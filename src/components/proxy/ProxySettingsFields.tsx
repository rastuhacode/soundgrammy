import { useEffect, useState } from 'react'
import { api } from '@/lib/api'
import { buildProxyLink } from '@/lib/proxy-link'
import type { ProxySettings, ProxySettingsView } from '@/types'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'

function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string' && message.length > 0) return message
  }
  if (error instanceof Error && error.message) return error.message
  return 'Something went wrong'
}

export interface ProxySettingsFieldsProps {
  /** Compact layout for the login screen. */
  compact?: boolean
  /** Called after a successful apply (reconnect). */
  onApplied?: (view: ProxySettingsView) => void
}

export function ProxySettingsFields({
  compact = false,
  onApplied,
}: ProxySettingsFieldsProps) {
  const [enabled, setEnabled] = useState(false)
  const [server, setServer] = useState('')
  const [port, setPort] = useState('1443')
  const [secret, setSecret] = useState('')
  const [pasteLink, setPasteLink] = useState('')
  const [active, setActive] = useState(false)
  const [applyError, setApplyError] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [busyLabel, setBusyLabel] = useState<string | null>(null)
  const [loaded, setLoaded] = useState(false)

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const view = await api.getProxySettings()
        if (cancelled) return
        applyView(view)
        setLoaded(true)
      }
      catch (err) {
        if (!cancelled) {
          setError(errorMessage(err))
          setLoaded(true)
        }
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  function applyView(view: ProxySettingsView) {
    setEnabled(view.enabled)
    setServer(view.server)
    setPort(String(view.port || 1443))
    setSecret(view.secret)
    setPasteLink(view.link ?? '')
    setActive(view.active)
    setApplyError(view.applyError)
    setError(null)
  }

  async function handleParsePaste() {
    const raw = pasteLink.trim()
    if (!raw) return
    setBusy(true)
    setError(null)
    try {
      const parsed = await api.parseProxyLink(raw)
      setEnabled(true)
      setServer(parsed.server)
      setPort(String(parsed.port))
      setSecret(parsed.secret)
      setPasteLink(
        buildProxyLink(parsed.server, parsed.port, parsed.secret)
        || raw,
      )
    }
    catch (err) {
      setError(errorMessage(err))
    }
    finally {
      setBusy(false)
    }
  }

  function syncPasteFromFields(
    nextServer: string,
    nextPort: string,
    nextSecret: string,
  ) {
    const portNum = Number.parseInt(nextPort, 10)
    if (!Number.isFinite(portNum)) return
    const link = buildProxyLink(nextServer, portNum, nextSecret)
    if (link) setPasteLink(link)
  }

  async function handleApply(next?: Partial<ProxySettings>) {
    const portNum = Number.parseInt(next?.port?.toString() ?? port, 10)
    if (!Number.isFinite(portNum) || portNum <= 0 || portNum > 65535) {
      setError('Port must be between 1 and 65535')
      return
    }

    const payload: ProxySettings = {
      enabled: next?.enabled ?? enabled,
      server: (next?.server ?? server).trim(),
      port: portNum,
      secret: (next?.secret ?? secret).trim(),
    }

    setBusy(true)
    setBusyLabel(
      payload.enabled
        ? 'Connecting via MTProto proxy (up to ~20s)…'
        : 'Reconnecting directly (up to ~20s)…',
    )
    setError(null)
    try {
      const view = await api.setProxySettings(payload)
      applyView(view)
      onApplied?.(view)
    }
    catch (err) {
      setError(errorMessage(err))
      // Refresh status (may have applyError from a failed rebuild).
      try {
        const view = await api.getProxySettings()
        applyView(view)
      }
      catch {
        // ignore secondary fetch errors
      }
    }
    finally {
      setBusy(false)
      setBusyLabel(null)
    }
  }

  async function handleDisable() {
    await handleApply({ enabled: false })
  }

  if (!loaded) {
    return (
      <p className="text-xs text-muted-foreground">Loading proxy settings…</p>
    )
  }

  return (
    <div className={`flex min-w-0 flex-col ${compact ? 'gap-2.5' : 'gap-3'}`}>
      <label className="flex items-center gap-2 text-sm">
        <Checkbox
          checked={enabled}
          disabled={busy}
          onCheckedChange={(checked) => {
            setEnabled(checked === true)
          }}
        />
        <span>Use MTProto proxy</span>
      </label>

      <div className="flex min-w-0 flex-col gap-1.5">
        <Label htmlFor={compact ? 'proxy-link-login' : 'proxy-link'}>
          Paste link
        </Label>
        <Textarea
          id={compact ? 'proxy-link-login' : 'proxy-link'}
          rows={compact ? 2 : 3}
          placeholder="tg://proxy?server=127.0.0.1&port=1443&secret=…"
          value={pasteLink}
          disabled={busy}
          className="min-w-0 max-w-full break-all font-mono text-xs field-sizing-fixed resize-y"
          onChange={e => setPasteLink(e.target.value)}
          onBlur={() => {
            if (pasteLink.trim().includes('proxy?')) {
              handleParsePaste()
            }
          }}
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={busy || !pasteLink.trim()}
          onClick={handleParsePaste}
        >
          Parse link
        </Button>
      </div>

      <div className={`grid min-w-0 gap-3 ${compact ? 'grid-cols-1' : 'grid-cols-2'}`}>
        <div className="flex min-w-0 flex-col gap-1.5">
          <Label htmlFor={compact ? 'proxy-server-login' : 'proxy-server'}>
            Server
          </Label>
          <Input
            id={compact ? 'proxy-server-login' : 'proxy-server'}
            value={server}
            disabled={busy}
            placeholder="127.0.0.1"
            className="min-w-0 font-mono text-xs"
            onChange={(e) => {
              setServer(e.target.value)
              syncPasteFromFields(e.target.value, port, secret)
            }}
          />
        </div>
        <div className="flex min-w-0 flex-col gap-1.5">
          <Label htmlFor={compact ? 'proxy-port-login' : 'proxy-port'}>
            Port
          </Label>
          <Input
            id={compact ? 'proxy-port-login' : 'proxy-port'}
            type="number"
            min={1}
            max={65535}
            value={port}
            disabled={busy}
            className="min-w-0 font-mono text-xs"
            onChange={(e) => {
              setPort(e.target.value)
              syncPasteFromFields(server, e.target.value, secret)
            }}
          />
        </div>
      </div>

      <div className="flex min-w-0 flex-col gap-1.5">
        <Label htmlFor={compact ? 'proxy-secret-login' : 'proxy-secret'}>
          Secret
        </Label>
        <Input
          id={compact ? 'proxy-secret-login' : 'proxy-secret'}
          type="password"
          autoComplete="off"
          value={secret}
          disabled={busy}
          className="min-w-0 font-mono text-xs"
          onChange={(e) => {
            setSecret(e.target.value)
            syncPasteFromFields(server, port, e.target.value)
          }}
        />
      </div>

      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="secondary"
          size={compact ? 'sm' : 'default'}
          disabled={busy}
          onClick={() => handleApply()}
        >
          {busy ? 'Applying…' : 'Apply & reconnect'}
        </Button>
        {enabled || active
          ? (
              <Button
                type="button"
                variant="outline"
                size={compact ? 'sm' : 'default'}
                disabled={busy}
                onClick={handleDisable}
              >
                Disable proxy
              </Button>
            )
          : null}
      </div>

      {busyLabel
        ? (
            <p className="text-xs text-muted-foreground">{busyLabel}</p>
          )
        : null}

      <p className="text-xs text-muted-foreground">
        {active
          ? 'Proxy is active for Telegram traffic.'
          : enabled
            ? 'Proxy saved but not active — apply again or check the helper.'
            : ''}
      </p>

      {applyError
        ? (
            <p className="wrap-break-word text-xs text-destructive">
              Proxy issue:
              {' '}
              {applyError}
              {enabled
                ? ' — disable proxy or start tg-ws-proxy, then apply again.'
                : null}
            </p>
          )
        : null}
      {error
        ? (
            <p className="wrap-break-word text-sm text-destructive">{error}</p>
          )
        : null}
    </div>
  )
}
