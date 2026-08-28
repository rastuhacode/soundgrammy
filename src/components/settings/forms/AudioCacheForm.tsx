import { useEffect, useState } from 'react'
import { useForm } from '@tanstack/react-form'
import { z } from 'zod'
import { errorMessage, fieldErrors } from './settings-form'
import { Button } from '@/components/ui/button'
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldSet,
} from '@/components/ui/fieldset'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api'
import { useCacheStore } from '@/stores/cache-store'
import type { CacheSettings, CacheUsage } from '@/types'

const GIB = 1024 ** 3
const DAY_SECONDS = 86_400

const nonNegativeNumber = (message: string) => z.string().refine((value) => {
  const number = Number.parseFloat(value)
  return Number.isFinite(number) && number >= 0
}, message)

const cacheSettingsSchema = z.object({
  limitGb: nonNegativeNumber('Size limit must be a non-negative number'),
  ttlDays: nonNegativeNumber('Keep-for days must be a non-negative number'),
})

function formatBytes(bytes: number): string {
  if (bytes >= GIB) return `${(bytes / GIB).toFixed(2)} GB`
  if (bytes >= 1024 ** 2) return `${(bytes / (1024 ** 2)).toFixed(1)} MB`
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`
  return `${bytes} B`
}

function formValues(settings: CacheSettings) {
  return {
    limitGb: String(Math.round((settings.limitBytes / GIB) * 100) / 100),
    ttlDays: String(Math.round(settings.ttlSecs / DAY_SECONDS)),
  }
}

export function AudioCacheForm() {
  const [settings, setSettings] = useState<CacheSettings | null>(null)
  const [usage, setUsage] = useState<CacheUsage | null>(null)
  const [loaded, setLoaded] = useState(false)
  const [busy, setBusy] = useState(false)
  const [confirmClear, setConfirmClear] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const form = useForm({
    defaultValues: {
      limitGb: '5',
      ttlDays: '30',
    },
    validators: {
      onSubmit: cacheSettingsSchema,
    },
    onSubmit: async ({ value }) => {
      setBusy(true)
      setError(null)
      try {
        const next = await api.setCacheSettings({
          limitBytes: Math.round(Number.parseFloat(value.limitGb) * GIB),
          ttlSecs: Math.round(Number.parseFloat(value.ttlDays) * DAY_SECONDS),
        })
        setSettings(next)
        form.reset(formValues(next))
        setUsage(await api.getCacheUsage())
        await useCacheStore.getState().hydrate()
      }
      catch (err) {
        setError(errorMessage(err))
      }
      finally {
        setBusy(false)
      }
    },
  })

  useEffect(() => {
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
        form.reset(formValues(nextSettings))
        setError(null)
      }
      catch (err) {
        if (!cancelled) setError(errorMessage(err))
      }
      finally {
        if (!cancelled) setLoaded(true)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [form])

  const handleClear = async () => {
    setBusy(true)
    setError(null)
    try {
      await api.clearAudioCache()
      useCacheStore.getState().clearAll()
      setUsage(await api.getCacheUsage())
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
    <form
      onSubmit={(event) => {
        event.preventDefault()
        event.stopPropagation()
        void form.handleSubmit()
      }}
      noValidate
    >
      <FieldSet disabled={!loaded || busy}>
        <FieldDescription className="text-xs">
          Cached audio stays in the app and accessible offline, but takes up physical space on your device.
        </FieldDescription>

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

        <FieldGroup className="grid grid-cols-2 gap-3">
          <form.Field name="limitGb">
            {field => (
              <Field data-invalid={field.state.meta.errors.length > 0 || undefined}>
                <FieldLabel htmlFor={field.name}>Size limit (GB)</FieldLabel>
                <Input
                  id={field.name}
                  name={field.name}
                  type="number"
                  min={0}
                  step={0.5}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={event => field.handleChange(event.target.value)}
                  aria-invalid={field.state.meta.errors.length > 0 || undefined}
                />
                <FieldError errors={fieldErrors(field.state.meta.errors)} />
              </Field>
            )}
          </form.Field>
          <form.Field name="ttlDays">
            {field => (
              <Field data-invalid={field.state.meta.errors.length > 0 || undefined}>
                <FieldLabel htmlFor={field.name}>Keep for (days)</FieldLabel>
                <Input
                  id={field.name}
                  name={field.name}
                  type="number"
                  min={0}
                  step={1}
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={event => field.handleChange(event.target.value)}
                  aria-invalid={field.state.meta.errors.length > 0 || undefined}
                />
                <FieldError errors={fieldErrors(field.state.meta.errors)} />
              </Field>
            )}
          </form.Field>
        </FieldGroup>

        <div className="flex flex-wrap gap-2">
          <Button type="submit" variant="secondary" disabled={!loaded || busy}>
            {busy ? 'Saving…' : 'Save cache settings'}
          </Button>
          {!confirmClear
            ? (
                <Button
                  type="button"
                  variant="outline"
                  disabled={!loaded || busy}
                  onClick={() => setConfirmClear(true)}
                >
                  Clear cache
                </Button>
              )
            : (
                <Button
                  type="button"
                  variant="destructive"
                  disabled={!loaded || busy}
                  onClick={handleClear}
                >
                  Confirm clear cache
                </Button>
              )}
        </div>
        {confirmClear
          ? <FieldDescription className="text-xs">This removes app-cached audio only.</FieldDescription>
          : null}
        <FieldError className="wrap-break-word">{error}</FieldError>
      </FieldSet>
    </form>
  )
}
