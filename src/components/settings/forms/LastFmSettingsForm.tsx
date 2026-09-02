import { useEffect, useId, useState } from 'react'
import { useForm } from '@tanstack/react-form'
import { Loader2 } from 'lucide-react'
import { z } from 'zod'
import { ExperementalBadge } from '@/components/badges/ExperementalBadge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from '@/components/ui/fieldset'
import { api } from '@/lib/api'
import { useLastFmStore } from '@/stores/lastfm-store'
import type { LastFmPendingAction, LastFmStatus } from '@/types'

const lastFmSettingsSchema = z.object({
  enabled: z.boolean(),
})

function formatTime(timestamp: number | null): string {
  if (timestamp == null) return 'Never'
  const elapsed = Date.now() - timestamp
  if (elapsed < 60_000) return 'Just now'
  if (elapsed < 3_600_000) return `${Math.floor(elapsed / 60_000)}m ago`
  if (elapsed < 86_400_000) return `${Math.floor(elapsed / 3_600_000)}h ago`
  return new Date(timestamp).toLocaleString()
}

export function LastFmSettingsForm() {
  const id = useId()
  const status = useLastFmStore(state => state.status)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [confirmDisconnect, setConfirmDisconnect] = useState(false)

  const form = useForm({
    defaultValues: {
      enabled: status?.enabled ?? false,
    },
    validators: {
      onSubmit: lastFmSettingsSchema,
    },
    onSubmit: async ({ value }) => {
      await run(() => api.setLastFmEnabled(value.enabled))
    },
  })

  const applyStatus = (next: LastFmStatus) => {
    useLastFmStore.getState().setStatus(next)
    form.reset({ enabled: next.enabled })
  }

  useEffect(() => {
    if (status) {
      form.reset({ enabled: status.enabled })
      return
    }
    useLastFmStore.getState().hydrate().catch(() => {
      setError('Last.fm status could not be loaded.')
    })
  }, [form, status])

  async function run(action: () => Promise<LastFmStatus>) {
    setBusy(true)
    setError(null)
    try {
      applyStatus(await action())
    }
    catch {
      if (status) form.reset({ enabled: status.enabled })
      setError('The Last.fm action could not be completed.')
    }
    finally {
      setBusy(false)
    }
  }

  const disconnect = async (pendingAction?: LastFmPendingAction) => {
    await run(() => api.disconnectLastFm(pendingAction))
    setConfirmDisconnect(false)
  }

  const currentRetained = status?.username
    ? status.retainedQueues.find(queue =>
        queue.username.localeCompare(status.username ?? '', undefined, { sensitivity: 'base' }) === 0)
    : null
  const otherRetained = status?.retainedQueues.filter(queue =>
    !status.username
    || queue.username.localeCompare(status.username, undefined, { sensitivity: 'base' }) !== 0)
  ?? []

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        event.stopPropagation()
        void form.handleSubmit()
      }}
    >
      <FieldSet disabled={busy}>
        <FieldLegend className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          <span>
            Last.fm
          </span>
          <ExperementalBadge />
        </FieldLegend>
        <FieldDescription className="text-xs">
          Send the music you listen to in SoundGrammy to your Last.fm profile. Tracks are added after half the track or four minutes, whichever comes first.
        </FieldDescription>

        {!status ? <Loader2 className="size-4 animate-spin text-muted-foreground" /> : null}

        {status?.state === 'unavailable_in_build'
          ? (
              <div className="flex flex-col items-start gap-2">
                <p className="text-sm text-muted-foreground">Last.fm is unavailable in this build.</p>
                <Button type="button" size="sm" disabled>Connect Last.fm</Button>
              </div>
            )
          : null}

        {status && ['disconnected', 'error'].includes(status.state) && !status.username
          ? (
              <div className="flex flex-col items-start gap-2">
                <Button type="button" size="sm" disabled={busy} onClick={() => run(api.startLastFmAuth)}>
                  Connect Last.fm
                </Button>
              </div>
            )
          : null}

        {status && ['requesting_token', 'exchanging_session'].includes(status.state)
          ? (
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <Loader2 className="size-4 animate-spin" />
                {status.state === 'requesting_token'
                  ? 'Starting Last.fm authorization…'
                  : 'Finishing Last.fm authorization…'}
              </div>
            )
          : null}

        {status?.state === 'waiting_for_browser_approval'
          ? (
              <div className="flex flex-col gap-2">
                <p className="text-sm">Authorize SoundGrammy in the browser.</p>
                <p className="flex items-center gap-2 text-xs text-muted-foreground">
                  <Loader2 className="size-3.5 animate-spin" />
                  Waiting for authorization…
                </p>
                <div className="flex flex-wrap gap-2">
                  <Button type="button" size="sm" disabled={busy} onClick={() => run(api.completeLastFmAuth)}>
                    I&apos;ve authorized SoundGrammy
                  </Button>
                  <Button type="button" variant="outline" size="sm" disabled={busy} onClick={() => run(api.cancelLastFmAuth)}>
                    Cancel
                  </Button>
                </div>
              </div>
            )
          : null}

        {status?.state === 'needs_reauthentication'
          ? (
              <div className="flex flex-col items-start gap-2">
                <p className="text-sm text-destructive">
                  Reconnect Last.fm to resume scrobbling for @
                  {status.username}
                  .
                </p>
                <div className="flex flex-wrap gap-2">
                  <Button type="button" size="sm" disabled={busy} onClick={() => run(api.startLastFmAuth)}>
                    Reconnect Last.fm
                  </Button>
                  <Button type="button" variant="outline" size="sm" disabled={busy} onClick={() => setConfirmDisconnect(true)}>
                    Disconnect
                  </Button>
                </div>
              </div>
            )
          : null}

        {status?.state === 'error' && status.username
          ? (
              <div className="flex flex-col items-start gap-2">
                <p className="text-sm text-destructive">
                  Last.fm needs attention for @
                  {status.username}
                  .
                </p>
                <div className="flex flex-wrap gap-2">
                  <Button type="button" size="sm" disabled={busy} onClick={() => run(api.startLastFmAuth)}>Reconnect Last.fm</Button>
                  <Button type="button" variant="outline" size="sm" disabled={busy} onClick={() => setConfirmDisconnect(true)}>Disconnect</Button>
                </div>
              </div>
            )
          : null}

        {status?.state === 'connected'
          ? (
              <div className="flex flex-col gap-3">
                <p className="flex items-center gap-2 text-sm">
                  <span className="size-2 rounded-full bg-emerald-500" />
                  Connected as @
                  {status.username}
                </p>
                <form.Field name="enabled">
                  {field => (
                    <Field orientation="horizontal">
                      <Checkbox
                        id={`${id}-enabled`}
                        name={field.name}
                        checked={field.state.value}
                        disabled={busy}
                        onBlur={field.handleBlur}
                        onCheckedChange={(checked) => {
                          field.handleChange(checked === true)
                          void form.handleSubmit()
                        }}
                      />
                      <FieldLabel htmlFor={`${id}-enabled`}>Scrobble SoundGrammy listening</FieldLabel>
                    </Field>
                  )}
                </form.Field>
                {!status.enabled && status.pendingCount > 0
                  ? (
                      <FieldDescription className="text-xs">
                        Pending uploads are retained and will resume when scrobbling is enabled.
                      </FieldDescription>
                    )
                  : null}
                <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
                  <dt className="text-muted-foreground">Last scrobble</dt>
                  <dd>{formatTime(status.lastScrobbleAtMs)}</dd>
                  <dt className="text-muted-foreground">Waiting to upload</dt>
                  <dd>{status.pendingCount}</dd>
                </dl>
                <div className="flex flex-wrap gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={busy}
                    onClick={() => api.openLastFmProfile().catch(() => setError('The Last.fm profile could not be opened.'))}
                  >
                    Open Last.fm profile
                  </Button>
                  <Button type="button" variant="outline" size="sm" disabled={busy} onClick={() => setConfirmDisconnect(true)}>
                    Disconnect
                  </Button>
                </div>
              </div>
            )
          : null}

        {confirmDisconnect && status
          ? (
              <div className="rounded-md border border-border/70 bg-muted/30 p-3 text-xs">
                {status.pendingCount > 0 || (currentRetained?.count ?? 0) > 0
                  ? (
                      <>
                        <p className="mb-2">
                          Keep pending scrobbles for @
                          {status.username}
                          , or delete them permanently?
                        </p>
                        <div className="flex flex-wrap gap-2">
                          <Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => disconnect('retain')}>Retain and disconnect</Button>
                          <Button type="button" size="sm" variant="destructive" disabled={busy} onClick={() => disconnect('delete')}>Delete and disconnect</Button>
                          <Button type="button" size="sm" variant="ghost" disabled={busy} onClick={() => setConfirmDisconnect(false)}>Cancel</Button>
                        </div>
                      </>
                    )
                  : (
                      <div className="flex flex-wrap gap-2">
                        <Button type="button" size="sm" variant="destructive" disabled={busy} onClick={() => disconnect()}>Confirm disconnect</Button>
                        <Button type="button" size="sm" variant="ghost" disabled={busy} onClick={() => setConfirmDisconnect(false)}>Cancel</Button>
                      </div>
                    )}
              </div>
            )
          : null}

        {otherRetained.length > 0
          ? (
              <FieldDescription className="text-xs">
                Retained for another account:
                {' '}
                {otherRetained.map(queue => `@${queue.username} (${queue.count})`).join(', ')}
                . These will upload only after reconnecting that account.
              </FieldDescription>
            )
          : null}

        {status?.lastError ? <FieldError className="text-xs">{status.lastError.message}</FieldError> : null}
        {status?.lastMetadataWarning
          ? <FieldDescription className="text-xs">{status.lastMetadataWarning.message}</FieldDescription>
          : null}
        <FieldError className="text-xs">{error}</FieldError>
      </FieldSet>
    </form>
  )
}
