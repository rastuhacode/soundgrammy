import { useEffect, useState } from 'react'
import { ChevronDown } from 'lucide-react'
import { api } from '@/lib/api'
import type { CacheSettings, CacheUsage } from '@/types'
import { useCacheStore } from '@/stores/cache-store'
import { useFullscreenStore } from '@/stores/fullscreen-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { useLogStore } from '@/stores/log-store'
import { ProxySettingsFields } from '@/components/proxy/ProxySettingsFields'
import { LogsDialog } from '@/components/LogsDialog'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Slider } from '@/components/ui/slider'
import { ExperementalBadge } from './badges/ExperementalBadge'

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
  const [confirmClearStatistics, setConfirmClearStatistics] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [cacheOpen, setCacheOpen] = useState(false)
  const [proxyOpen, setProxyOpen] = useState(false)
  const [fullscreenOpen, setFullscreenOpen] = useState(false)
  const [bounceOpen, setBounceOpen] = useState(false)
  const [statisticsOpen, setStatisticsOpen] = useState(false)
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false)
  const [logsOpen, setLogsOpen] = useState(false)
  const statisticsEnabled = useListenStatsStore(state => state.enabled)
  const setStatisticsEnabled = useListenStatsStore(state => state.setEnabled)
  const bounce = useFullscreenStore(state => state.bounce)
  const keepDisplayAwake = useFullscreenStore(state => state.keepDisplayAwake)
  const setKeepDisplayAwake = useFullscreenStore(state => state.setKeepDisplayAwake)
  const setBounceSettings = useFullscreenStore(state => state.setBounceSettings)
  const resetBounceSettings = useFullscreenStore(state => state.resetBounceSettings)
  const diagnosticLogsEnabled = useLogStore(state => state.enabled)
  const diagnosticLogCount = useLogStore(state => state.entries.length)
  const setDiagnosticLogsEnabled = useLogStore(state => state.setEnabled)

  useEffect(() => {
    if (!open) return

    let cancelled = false
    ;(async () => {
      try {
        const [nextSettings, nextUsage, nextStatisticsEnabled] = await Promise.all([
          api.getCacheSettings(),
          api.getCacheUsage(),
          api.getListenStatisticsEnabled(),
        ])
        if (cancelled) return
        setSettings(nextSettings)
        setUsage(nextUsage)
        setLimitGb(String(Math.round((nextSettings.limitBytes / GIB) * 100) / 100))
        setTtlDays(String(Math.round(nextSettings.ttlSecs / 86_400)))
        setConfirmClear(false)
        setConfirmClearStatistics(false)
        setStatisticsEnabled(nextStatisticsEnabled)
        setError(null)
        setCacheOpen(false)
        setProxyOpen(false)
        setFullscreenOpen(false)
        setBounceOpen(false)
        setStatisticsOpen(false)
        setDiagnosticsOpen(false)
      }
      catch (err) {
        if (!cancelled) setError(errorMessage(err))
      }
    })()

    return () => {
      cancelled = true
    }
  }, [open, setStatisticsEnabled])

  const handleOpenChange = (next: boolean) => {
    if (!next) {
      setConfirmClear(false)
      setConfirmClearStatistics(false)
      setError(null)
      setCacheOpen(false)
      setProxyOpen(false)
      setFullscreenOpen(false)
      setBounceOpen(false)
      setStatisticsOpen(false)
      setDiagnosticsOpen(false)
      setLogsOpen(false)
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

  const handleStatisticsEnabledChange = async (enabled: boolean) => {
    setBusy(true)
    setError(null)
    try {
      await api.setListenStatisticsEnabled(enabled)
      setStatisticsEnabled(enabled)
    }
    catch (err) {
      setError(errorMessage(err))
    }
    finally {
      setBusy(false)
    }
  }

  const handleClearStatistics = async () => {
    setBusy(true)
    setError(null)
    try {
      await api.clearListenStatistics()
      useListenStatsStore.getState().clear()
      setConfirmClearStatistics(false)
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
    <>
      <Dialog open={open} onOpenChange={handleOpenChange}>
        <DialogContent className="flex max-h-[90vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-lg">
          <DialogHeader className="shrink-0 border-b border-border/60 px-4 py-4 pr-12">
            <DialogTitle>Settings</DialogTitle>
          </DialogHeader>

          <div className="min-h-0 grow overflow-x-hidden overflow-y-auto px-4 py-4">
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
                            Clear cache
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

              <Collapsible open={statisticsOpen} onOpenChange={setStatisticsOpen}>
                <CollapsibleTrigger className="flex h-auto w-full items-center justify-between rounded-md px-2 py-2.5 text-sm font-medium hover:bg-muted/40">
                  <span>Listening statistics</span>
                  <ChevronDown
                    className={`size-4 text-muted-foreground transition-transform ${statisticsOpen ? 'rotate-180' : ''}`}
                  />
                </CollapsibleTrigger>
                <CollapsibleContent className="mt-1 flex flex-col gap-3 px-2 pb-3">
                  <p className="text-xs text-muted-foreground">
                    Track your listening activity to build the Popular and Recent playlists. Turning this off keeps existing history but hides those playlists.
                  </p>

                  <label className="flex items-center gap-2 text-sm">
                    <Checkbox
                      checked={statisticsEnabled}
                      disabled={busy}
                      onCheckedChange={checked => handleStatisticsEnabledChange(checked === true)}
                    />
                    <span>Collect listening statistics</span>
                  </label>

                  <div>
                    {!confirmClearStatistics
                      ? (
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={busy}
                            onClick={() => setConfirmClearStatistics(true)}
                          >
                            Clear statistics
                          </Button>
                        )
                      : (
                          <Button
                            variant="destructive"
                            size="sm"
                            disabled={busy}
                            onClick={handleClearStatistics}
                          >
                            Confirm clear statistics
                          </Button>
                        )}
                  </div>
                  {confirmClearStatistics
                    ? (
                        <p className="text-xs text-muted-foreground">
                          This permanently removes all listening history from this device. Popular and Recent will be emptied.
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
                    Route Telegram through a local helper such as&nbsp;
                    <a href="https://github.com/Flowseal/tg-ws-proxy" target="_blank" rel="noopener noreferrer" className="underline">tg-ws-proxy</a>
                    . This could be helpful to avoid censorship.
                  </p>
                  {open && proxyOpen ? <ProxySettingsFields /> : null}
                </CollapsibleContent>
              </Collapsible>

              <Collapsible open={diagnosticsOpen} onOpenChange={setDiagnosticsOpen}>
                <CollapsibleTrigger className="flex h-auto w-full items-center justify-between rounded-md px-2 py-2.5 text-sm font-medium hover:bg-muted/40">
                  <span>Diagnostics</span>
                  <ChevronDown
                    className={`size-4 text-muted-foreground transition-transform ${diagnosticsOpen ? 'rotate-180' : ''}`}
                  />
                </CollapsibleTrigger>
                <CollapsibleContent className="mt-1 flex flex-col gap-3 px-2 pb-3">
                  <p className="text-xs text-muted-foreground">
                    Keep a local history of application errors for troubleshooting. Logs may contain track identifiers and local file paths.
                  </p>

                  <label className="flex items-center gap-2 text-sm">
                    <Checkbox
                      checked={diagnosticLogsEnabled}
                      onCheckedChange={checked => setDiagnosticLogsEnabled(checked === true)}
                    />
                    <span>Enable diagnostic logs</span>
                  </label>

                  <div>
                    <Button variant="outline" size="sm" onClick={() => setLogsOpen(true)}>
                      View logs
                      {diagnosticLogCount > 0 ? ` (${diagnosticLogCount})` : ''}
                    </Button>
                  </div>
                </CollapsibleContent>
              </Collapsible>

              <Collapsible open={fullscreenOpen} onOpenChange={setFullscreenOpen}>
                <CollapsibleTrigger className="flex h-auto w-full items-center justify-between rounded-md px-2 py-2.5 text-sm font-medium hover:bg-muted/40">
                  <span>Fullscreen player</span>
                  <ChevronDown
                    className={`size-4 text-muted-foreground transition-transform ${fullscreenOpen ? 'rotate-180' : ''}`}
                  />
                </CollapsibleTrigger>
                <CollapsibleContent className="mt-1 flex flex-col gap-3 px-2 pb-3">
                  <p className="text-xs text-muted-foreground">
                    Prevent normal inactivity from dimming or turning off the display while the fullscreen player is open. This may increase battery usage.
                  </p>

                  <label className="flex items-center gap-2 text-sm">
                    <Checkbox
                      checked={keepDisplayAwake}
                      onCheckedChange={checked => setKeepDisplayAwake(checked === true)}
                    />
                    <span>Keep display awake</span>
                  </label>
                </CollapsibleContent>
              </Collapsible>

              <Collapsible open={bounceOpen} onOpenChange={setBounceOpen}>
                <CollapsibleTrigger className="flex h-auto w-full items-center justify-between rounded-md px-2 py-2.5 text-sm font-medium hover:bg-muted/40">
                  <div className="flex items-center gap-2">
                    <span>Fullscreen bounce</span>
                    <ExperementalBadge />
                  </div>
                  <ChevronDown
                    className={`size-4 text-muted-foreground transition-transform ${bounceOpen ? 'rotate-180' : ''}`}
                  />
                </CollapsibleTrigger>
                <CollapsibleContent className="mt-1 flex flex-col gap-4 px-2 pb-3">
                  <p className="text-xs text-muted-foreground">
                    Move fullscreen artwork with the track’s overall dynamics and rhythmic accents.
                  </p>

                  <label className="flex items-center gap-2 text-sm">
                    <Checkbox
                      checked={bounce.enabled}
                      onCheckedChange={checked => setBounceSettings({ enabled: checked === true })}
                    />
                    <span>Enable artwork bounce</span>
                  </label>

                  <BounceSlider
                    label="Strength"
                    value={bounce.strength}
                    disabled={!bounce.enabled}
                    onChange={strength => setBounceSettings({ strength })}
                  />
                  <BounceSlider
                    label="Dynamics ↔ Beats"
                    value={bounce.balance}
                    disabled={!bounce.enabled}
                    onChange={balance => setBounceSettings({ balance })}
                    left="Dynamics"
                    right="Beats"
                  />
                  <BounceSlider
                    label="Smoothness"
                    value={bounce.smoothness}
                    disabled={!bounce.enabled}
                    onChange={smoothness => setBounceSettings({ smoothness })}
                    left="Snappy"
                    right="Fluid"
                  />

                  <div>
                    <Button variant="outline" size="sm" onClick={resetBounceSettings}>
                      Reset defaults
                    </Button>
                  </div>
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
      <LogsDialog open={logsOpen} onOpenChange={setLogsOpen} />
    </>
  )
}

function BounceSlider(props: {
  label: string
  value: number
  disabled: boolean
  onChange: (value: number) => void
  left?: string
  right?: string
}) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-3 text-sm">
        <Label>{props.label}</Label>
        <span className="font-mono text-xs tabular-nums text-muted-foreground">
          {props.value}
          %
        </span>
      </div>
      <Slider
        min={0}
        max={100}
        step={1}
        value={props.value}
        disabled={props.disabled}
        onValueChange={value => props.onChange(Number(value))}
        aria-label={props.label}
      />
      {props.left && props.right
        ? (
            <div className="flex justify-between text-[10px] uppercase tracking-wide text-muted-foreground">
              <span>{props.left}</span>
              <span>{props.right}</span>
            </div>
          )
        : null}
    </div>
  )
}
