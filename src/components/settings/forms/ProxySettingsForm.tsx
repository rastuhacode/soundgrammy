import { useCallback, useEffect, useId, useState } from 'react'
import { useForm } from '@tanstack/react-form'
import { z } from 'zod'
import { errorMessage, fieldErrors } from './settings-form'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from '@/components/ui/fieldset'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { api } from '@/lib/api'
import { buildProxyLink } from '@/lib/proxy-link'
import type { ProxySettings, ProxySettingsView } from '@/types'
import { TauriLink } from '@/components/tauri/TauriLink'

const proxySettingsSchema = z.object({
  enabled: z.boolean(),
  server: z.string(),
  port: z.string().refine((value) => {
    const port = Number(value)
    return Number.isInteger(port) && port >= 1 && port <= 65535
  }, 'Port must be between 1 and 65535'),
  secret: z.string(),
  pasteLink: z.string(),
}).superRefine((value, context) => {
  if (!value.enabled) return
  if (!value.server.trim()) {
    context.addIssue({
      code: 'custom',
      path: ['server'],
      message: 'Server is required when the proxy is enabled',
    })
  }
  if (!value.secret.trim()) {
    context.addIssue({
      code: 'custom',
      path: ['secret'],
      message: 'Secret is required when the proxy is enabled',
    })
  }
})

type ProxyFormValues = z.infer<typeof proxySettingsSchema>

export interface ProxySettingsFormProps {
  /** Compact layout used on the login screen. */
  compact?: boolean
  /** Called after a successful apply (reconnect). */
  onApplied?: (view: ProxySettingsView) => void
}

export function ProxySettingsForm({
  compact = false,
  onApplied,
}: ProxySettingsFormProps) {
  const id = useId()
  const [active, setActive] = useState(false)
  const [applyError, setApplyError] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [busyLabel, setBusyLabel] = useState<string | null>(null)
  const [loaded, setLoaded] = useState(false)

  const form = useForm({
    defaultValues: {
      enabled: false,
      server: '',
      port: '1443',
      secret: '',
      pasteLink: '',
    },
    validators: {
      onSubmit: proxySettingsSchema,
    },
    onSubmit: async ({ value }) => {
      await applyProxy(value)
    },
  })

  const applyView = useCallback((view: ProxySettingsView) => {
    form.reset({
      enabled: view.enabled,
      server: view.server,
      port: String(view.port || 1443),
      secret: view.secret,
      pasteLink: view.link ?? '',
    })
    setActive(view.active)
    setApplyError(view.applyError)
    setError(null)
  }, [form])

  useEffect(() => {
    let cancelled = false
    api.getProxySettings()
      .then((view) => {
        if (!cancelled) applyView(view)
      })
      .catch((err) => {
        if (!cancelled) setError(errorMessage(err))
      })
      .finally(() => {
        if (!cancelled) setLoaded(true)
      })
    return () => {
      cancelled = true
    }
  }, [applyView])

  async function applyProxy(values: ProxyFormValues) {
    const payload: ProxySettings = {
      enabled: values.enabled,
      server: values.server.trim(),
      port: Number.parseInt(values.port, 10),
      secret: values.secret.trim(),
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
      try {
        applyView(await api.getProxySettings())
      }
      catch {
        // Ignore a secondary status refresh failure.
      }
    }
    finally {
      setBusy(false)
      setBusyLabel(null)
    }
  }

  const handleParsePaste = async () => {
    const raw = form.state.values.pasteLink.trim()
    if (!raw) return
    setBusy(true)
    setError(null)
    try {
      const parsed = await api.parseProxyLink(raw)
      form.setFieldValue('enabled', true)
      form.setFieldValue('server', parsed.server)
      form.setFieldValue('port', String(parsed.port))
      form.setFieldValue('secret', parsed.secret)
      form.setFieldValue(
        'pasteLink',
        buildProxyLink(parsed.server, parsed.port, parsed.secret) || raw,
      )
    }
    catch (err) {
      setError(errorMessage(err))
    }
    finally {
      setBusy(false)
    }
  }

  const syncPasteFromFields = (values: {
    server?: string
    port?: string
    secret?: string
  }) => {
    const current = form.state.values
    const server = values.server ?? current.server
    const port = values.port ?? current.port
    const secret = values.secret ?? current.secret
    const portNumber = Number.parseInt(port, 10)
    if (!Number.isFinite(portNumber)) return
    const link = buildProxyLink(server, portNumber, secret)
    if (link) form.setFieldValue('pasteLink', link)
  }

  const handleDisable = async () => {
    const values = { ...form.state.values, enabled: false }
    const parsed = proxySettingsSchema.safeParse(values)
    if (!parsed.success) return
    form.setFieldValue('enabled', false)
    await applyProxy(parsed.data)
  }

  const fields = (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        event.stopPropagation()
        void form.handleSubmit()
      }}
      noValidate
    >
      <FieldSet disabled={!loaded || busy} className={compact ? 'gap-2.5' : 'gap-3'}>
        {!compact
          ? (
              <>
                <FieldLegend className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  MTProto proxy
                </FieldLegend>
                <FieldDescription className="text-xs">
                  Route Telegram through a local helper such as&nbsp;
                  <TauriLink href="https://github.com/Flowseal/tg-ws-proxy">tg-ws-proxy</TauriLink>
                  . This could be helpful to avoid censorship.
                </FieldDescription>
              </>
            )
          : null}

        <form.Field name="enabled">
          {field => (
            <Field orientation="horizontal">
              <Checkbox
                id={`${id}-enabled`}
                name={field.name}
                checked={field.state.value}
                disabled={!loaded || busy}
                onBlur={field.handleBlur}
                onCheckedChange={checked => field.handleChange(checked === true)}
              />
              <FieldLabel htmlFor={`${id}-enabled`}>Use MTProto proxy</FieldLabel>
            </Field>
          )}
        </form.Field>

        <form.Field name="pasteLink">
          {field => (
            <Field>
              <FieldLabel htmlFor={`${id}-link`}>Paste link</FieldLabel>
              <Textarea
                id={`${id}-link`}
                name={field.name}
                rows={compact ? 2 : 3}
                placeholder="tg://proxy?server=127.0.0.1&port=1443&secret=…"
                value={field.state.value}
                disabled={!loaded || busy}
                className="min-w-0 max-w-full break-all font-mono text-xs field-sizing-fixed resize-y"
                onChange={event => field.handleChange(event.target.value)}
                onBlur={() => {
                  field.handleBlur()
                  if (field.state.value.trim().includes('proxy?')) void handleParsePaste()
                }}
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={!loaded || busy || !field.state.value.trim()}
                onClick={handleParsePaste}
              >
                Parse link
              </Button>
            </Field>
          )}
        </form.Field>

        <FieldGroup className={`grid min-w-0 gap-3 ${compact ? 'grid-cols-1' : 'grid-cols-2'}`}>
          <form.Field name="server">
            {field => (
              <Field className="min-w-0" data-invalid={field.state.meta.errors.length > 0 || undefined}>
                <FieldLabel htmlFor={`${id}-server`}>Server</FieldLabel>
                <Input
                  id={`${id}-server`}
                  name={field.name}
                  value={field.state.value}
                  disabled={!loaded || busy}
                  placeholder="127.0.0.1"
                  className="min-w-0 font-mono text-xs"
                  onBlur={field.handleBlur}
                  onChange={(event) => {
                    field.handleChange(event.target.value)
                    syncPasteFromFields({ server: event.target.value })
                  }}
                  aria-invalid={field.state.meta.errors.length > 0 || undefined}
                />
                <FieldError errors={fieldErrors(field.state.meta.errors)} />
              </Field>
            )}
          </form.Field>
          <form.Field name="port">
            {field => (
              <Field className="min-w-0" data-invalid={field.state.meta.errors.length > 0 || undefined}>
                <FieldLabel htmlFor={`${id}-port`}>Port</FieldLabel>
                <Input
                  id={`${id}-port`}
                  name={field.name}
                  type="number"
                  min={1}
                  max={65535}
                  value={field.state.value}
                  disabled={!loaded || busy}
                  className="min-w-0 font-mono text-xs"
                  onBlur={field.handleBlur}
                  onChange={(event) => {
                    field.handleChange(event.target.value)
                    syncPasteFromFields({ port: event.target.value })
                  }}
                  aria-invalid={field.state.meta.errors.length > 0 || undefined}
                />
                <FieldError errors={fieldErrors(field.state.meta.errors)} />
              </Field>
            )}
          </form.Field>
        </FieldGroup>

        <form.Field name="secret">
          {field => (
            <Field className="min-w-0" data-invalid={field.state.meta.errors.length > 0 || undefined}>
              <FieldLabel htmlFor={`${id}-secret`}>Secret</FieldLabel>
              <Input
                id={`${id}-secret`}
                name={field.name}
                type="password"
                autoComplete="off"
                value={field.state.value}
                disabled={!loaded || busy}
                className="min-w-0 font-mono text-xs"
                onBlur={field.handleBlur}
                onChange={(event) => {
                  field.handleChange(event.target.value)
                  syncPasteFromFields({ secret: event.target.value })
                }}
                aria-invalid={field.state.meta.errors.length > 0 || undefined}
              />
              <FieldError errors={fieldErrors(field.state.meta.errors)} />
            </Field>
          )}
        </form.Field>

        <form.Subscribe selector={state => state.values.enabled}>
          {enabled => (
            <>
              <div className="flex flex-wrap gap-2">
                <Button
                  type="submit"
                  variant="secondary"
                  size={compact ? 'sm' : 'default'}
                  disabled={!loaded || busy}
                >
                  {busy ? 'Applying…' : 'Apply & reconnect'}
                </Button>
                {enabled || active
                  ? (
                      <Button
                        type="button"
                        variant="outline"
                        size={compact ? 'sm' : 'default'}
                        disabled={!loaded || busy}
                        onClick={handleDisable}
                      >
                        Disable proxy
                      </Button>
                    )
                  : null}
              </div>

              {busyLabel ? <FieldDescription className="text-xs">{busyLabel}</FieldDescription> : null}
              <FieldDescription className="text-xs">
                {active
                  ? 'Proxy is active for Telegram traffic.'
                  : enabled
                    ? 'Proxy saved but not active — apply again or check the helper.'
                    : ''}
              </FieldDescription>
              {applyError
                ? (
                    <FieldError className="wrap-break-word text-xs">
                      Proxy issue:
                      {' '}
                      {applyError}
                      {enabled
                        ? ' — disable proxy or start tg-ws-proxy, then apply again.'
                        : null}
                    </FieldError>
                  )
                : null}
            </>
          )}
        </form.Subscribe>
        <FieldError className="wrap-break-word">{error}</FieldError>
      </FieldSet>
    </form>
  )

  return fields
}
