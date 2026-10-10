import { useEffect, useId, useState } from 'react'
import { useForm } from '@tanstack/react-form'
import { z } from 'zod'
import { errorMessage } from './settings-form'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
  FieldSet,
} from '@/components/ui/fieldset'
import { captureSession, isSessionCurrent } from '@/stores/session-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'

const listeningStatisticsSchema = z.object({
  enabled: z.boolean(),
})

export function ListeningStatisticsForm() {
  const id = useId()
  const statisticsEnabled = useListenStatsStore(state => state.enabled)
  const refreshEnabled = useListenStatsStore(state => state.refreshEnabled)
  const [loaded, setLoaded] = useState(false)
  const [busy, setBusy] = useState(false)
  const [confirmClear, setConfirmClear] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const form = useForm({
    defaultValues: {
      enabled: statisticsEnabled,
    },
    validators: {
      onSubmit: listeningStatisticsSchema,
    },
    onSubmit: async ({ value }) => {
      const generation = captureSession()
      setBusy(true)
      setError(null)
      try {
        await useListenStatsStore.getState().saveEnabled(value.enabled)
      }
      catch (err) {
        if (!isSessionCurrent(generation)) return
        form.reset({ enabled: useListenStatsStore.getState().enabled })
        setError(errorMessage(err))
      }
      finally {
        if (isSessionCurrent(generation)) setBusy(false)
      }
    },
  })

  useEffect(() => {
    let cancelled = false
    refreshEnabled()
      .then((enabled) => {
        if (cancelled) return
        form.reset({ enabled })
        setError(null)
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
  }, [form, refreshEnabled])

  const handleClear = async () => {
    const generation = captureSession()
    setBusy(true)
    setError(null)
    try {
      await useListenStatsStore.getState().clearHistory()
      if (!isSessionCurrent(generation)) return
      setConfirmClear(false)
    }
    catch (err) {
      if (!isSessionCurrent(generation)) return
      setError(errorMessage(err))
    }
    finally {
      if (isSessionCurrent(generation)) setBusy(false)
    }
  }

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        event.stopPropagation()
        void form.handleSubmit()
      }}
    >
      <FieldSet disabled={!loaded || busy}>
        <FieldDescription className="text-xs">
          Track your listening activity to build the Popular and Recent playlists. Turning this off keeps existing history but hides those playlists.
        </FieldDescription>

        <form.Field name="enabled">
          {field => (
            <Field orientation="horizontal">
              <Checkbox
                id={`${id}-enabled`}
                name={field.name}
                checked={field.state.value}
                disabled={!loaded || busy}
                onBlur={field.handleBlur}
                onCheckedChange={(checked) => {
                  field.handleChange(checked === true)
                  void form.handleSubmit()
                }}
              />
              <FieldLabel htmlFor={`${id}-enabled`}>Collect listening statistics</FieldLabel>
            </Field>
          )}
        </form.Field>

        <div>
          {!confirmClear
            ? (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={!loaded || busy}
                  onClick={() => setConfirmClear(true)}
                >
                  Clear statistics
                </Button>
              )
            : (
                <Button
                  type="button"
                  variant="destructive"
                  size="sm"
                  disabled={!loaded || busy}
                  onClick={handleClear}
                >
                  Confirm clear statistics
                </Button>
              )}
        </div>
        {confirmClear
          ? (
              <FieldDescription className="text-xs">
                This permanently removes all listening history from this device. Popular and Recent will be emptied.
              </FieldDescription>
            )
          : null}
        <FieldError className="wrap-break-word">{error}</FieldError>
      </FieldSet>
    </form>
  )
}
