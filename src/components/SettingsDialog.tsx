import { useEffect, useState } from 'react'
import { ChevronDown } from 'lucide-react'
import { api } from '@/lib/api'
import type { CacheSettings, CacheUsage } from '@/types'
import { useCacheStore } from '@/stores/cache-store'
import { ProxySettingsFields } from '@/components/proxy/ProxySettingsFields'
import { Button } from '@/components/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

const GIB = 1024 ** 3

function formatBytes(bytes: number): string {
  if (bytes >= GIB) return `${(bytes / GIB).toFixed(2)} GB`
  if (bytes >= 1024 ** 2) return `${(bytes / (1024 ** 2)).toFixed(1)} MB`
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`
  return `${bytes} B`
}

function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string' && message.length > 0) return message
  }
  if (error instanceof Error && error.message) return error.message
  return 'Something went wrong'
}

export interface SettingsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const [settings, setSettings] = useState<CacheSettings | null>(null)
  const [usage, setUsage] = useState<CacheUsage | null>(null)
  const [limitGb, setLimitGb] = useState('5')
  const [ttlDays, setTtlDays] = useState('30')
  const [busy, setBusy] = useState(false)
  const [confirmClear, setConfirmClear] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [cacheOpen, setCacheOpen] = useState(false)
  const [proxyOpen, setProxyOpen] = useState(false)

  useEffect(() => {
    if (!open) return

    let cancelled = false
    ;(async () => {
      try {
        const [nextSettings, nextUsage] = await Promise.all([
          api.getCacheSettings(),
          api.getCacheUsage(),
        ])
        if (cancelled) return
        setSettings(nextSettings)
        setUsage(nextUsage)
        setLimitGb(String(Math.round((nextSettings.limitBytes / GIB) * 100) / 100))
        setTtlDays(String(Math.round(nextSettings.ttlSecs / 86_400)))
        setConfirmClear(false)
        setError(null)
        setCacheOpen(false)
        setProxyOpen(false)
      }
      catch (err) {
        if (!cancelled) setError(errorMessage(err))
      }
    })()

    return () => {
      cancelled = true
    }
  }, [open])

  const handleOpenChange = (next: boolean) => {
    if (!next) {
      setConfirmClear(false)
      setError(null)
      setCacheOpen(false)
      setProxyOpen(false)
    }
    onOpenChange(next)
  }

  const handleSave = async () => {
    const limit = Number.parseFloat(limitGb)
    const days = Number.parseFloat(ttlDays)
    if (!Number.isFinite(limit) || limit < 0) {
      setError('Size limit must be a non-negative number')
      return
    }
    if (!Number.isFinite(days) || days < 0) {
      setError('Keep-for days must be a non-negative number')
      return
    }

    setBusy(true)
    setError(null)
    try {
      const next = await api.setCacheSettings({
        limitBytes: Math.round(limit * GIB),
        ttlSecs: Math.round(days * 86_400),
      })
      setSettings(next)
      const nextUsage = await api.getCacheUsage()
      setUsage(nextUsage)
      await useCacheStore.getState().hydrate()
    }
    catch (err) {
      setError(errorMessage(err))
    }
    finally {
      setBusy(false)
    }
  }

  const handleClear = async () => {
    setBusy(true)
    setError(null)
    try {
      await api.clearAudioCache()
      useCacheStore.getState().clearAll()
      const nextUsage = await api.getCacheUsage()
      setUsage(nextUsage)
      setConfirmClear(false)
    }
    catch (err) {
      setError(errorMessage(err))
    }
    finally {
      setBusy(false)
    }
  }

  const used = usage?.usedBytes ?? 0
  const limit = usage?.limitBytes ?? settings?.limitBytes ?? 0
  const pct = limit > 0 ? Math.min(100, Math.round((used / limit) * 100)) : 0

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="flex max-h-[90vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-lg">
        <DialogHeader className="shrink-0 border-b border-border/60 px-4 py-4 pr-12">
          <DialogTitle>Settings</DialogTitle>
        </DialogHeader>

        <div className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-4 py-4">
          <div className="flex min-w-0 flex-col gap-2">
            <Collapsible open={cacheOpen} onOpenChange={setCacheOpen}>
              <CollapsibleTrigger className="flex h-auto w-full items-center justify-between rounded-md px-2 py-2.5 text-sm font-medium hover:bg-muted/40">
                <span>Audio cache</span>
                <ChevronDown
                  className={`size-4 text-muted-foreground transition-transform ${cacheOpen ? 'rotate-180' : ''}`}
                />
              </CollapsibleTrigger>
              <CollapsibleContent className="mt-1 flex flex-col gap-3 px-2 pb-3">
                <p className="text-xs text-muted-foreground">
                  Cached audio stays in the app and accessible offline, but takes up physical space on your device.
                </p>

                <div className="rounded-lg border border-border/70 bg-muted/30 px-3 py-3">
                  <div className="flex items-baseline justify-between gap-3 text-sm">
                    <span className="text-muted-foreground">Usage</span>
                    <span className="font-mono text-xs">
                      {formatBytes(used)}
                      {' / '}
                      {formatBytes(limit)}
                      {' '}
                      (
                      {pct}
                      %)
                    </span>
                  </div>
                  <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-muted">
                    <div
                      className="h-full rounded-full bg-primary transition-[width] duration-300"
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                  {usage
                    ? (
                        <p className="mt-2 font-mono text-[10px] uppercase tracking-[0.12em] text-muted-foreground">
                          {usage.fileCount}
                          {' '}
                          file
                          {usage.fileCount === 1 ? '' : 's'}
                        </p>
                      )
                    : null}
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="flex flex-col gap-1.5">
                    <Label htmlFor="cache-limit">Size limit (GB)</Label>
                    <Input
                      id="cache-limit"
                      type="number"
                      min={0}
                      step={0.5}
                      value={limitGb}
                      onChange={e => setLimitGb(e.target.value)}
                      disabled={busy}
                    />
                  </div>
                  <div className="flex flex-col gap-1.5">
                    <Label htmlFor="cache-ttl">Keep for (days)</Label>
                    <Input
                      id="cache-ttl"
                      type="number"
                      min={0}
                      step={1}
                      value={ttlDays}
                      onChange={e => setTtlDays(e.target.value)}
                      disabled={busy}
                    />
                  </div>
                </div>

                <div className="flex flex-wrap gap-2">
                  <Button variant="secondary" disabled={busy} onClick={handleSave}>
                    Save cache settings
                  </Button>
                  {!confirmClear
                    ? (
                        <Button
                          variant="outline"
                          disabled={busy}
                          onClick={() => setConfirmClear(true)}
                        >
                          Clear cache…
                        </Button>
                      )
                    : (
                        <Button
                          variant="destructive"
                          disabled={busy}
                          onClick={handleClear}
                        >
                          Confirm clear cache
                        </Button>
                      )}
                </div>
                {confirmClear
                  ? (
                      <p className="text-xs text-muted-foreground">
                        This removes app-cached audio only.
                      </p>
                    )
                  : null}
              </CollapsibleContent>
            </Collapsible>

            <Collapsible open={proxyOpen} onOpenChange={setProxyOpen}>
              <CollapsibleTrigger className="flex h-auto w-full items-center justify-between rounded-md px-2 py-2.5 text-sm font-medium hover:bg-muted/40">
                <span>MTProto proxy</span>
                <ChevronDown
                  className={`size-4 text-muted-foreground transition-transform ${proxyOpen ? 'rotate-180' : ''}`}
                />
              </CollapsibleTrigger>
              <CollapsibleContent className="mt-1 flex min-w-0 flex-col gap-3 px-2 pb-3">
                <p className="text-xs text-muted-foreground">
                  Route Telegram through a local helper such as tg-ws-proxy
                  (
                  <span className="font-mono">127.0.0.1:1443</span>
                  ). Paste the link from the tray (“Copy link”), then Apply to
                  reconnect.
                </p>
                {open && proxyOpen ? <ProxySettingsFields /> : null}
              </CollapsibleContent>
            </Collapsible>

            {error
              ? (
                  <p className="wrap-break-word px-2 text-sm text-destructive">{error}</p>
                )
              : null}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
