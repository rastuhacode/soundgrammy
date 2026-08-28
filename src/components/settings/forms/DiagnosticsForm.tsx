import { useId, useState } from 'react'
import { useForm } from '@tanstack/react-form'
import { z } from 'zod'
import { LogsDialog } from '@/components/LogsDialog'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Field,
  FieldDescription,
  FieldLabel,
  FieldSet,
} from '@/components/ui/fieldset'
import { useLogStore } from '@/stores/log-store'

const diagnosticsSchema = z.object({
  enabled: z.boolean(),
})

export function DiagnosticsForm() {
  const id = useId()
  const diagnosticLogsEnabled = useLogStore(state => state.enabled)
  const diagnosticLogCount = useLogStore(state => state.entries.length)
  const setDiagnosticLogsEnabled = useLogStore(state => state.setEnabled)
  const [logsOpen, setLogsOpen] = useState(false)

  const form = useForm({
    defaultValues: {
      enabled: diagnosticLogsEnabled,
    },
    validators: {
      onSubmit: diagnosticsSchema,
    },
    onSubmit: ({ value }) => {
      setDiagnosticLogsEnabled(value.enabled)
    },
  })

  return (
    <>
      <form
        onSubmit={(event) => {
          event.preventDefault()
          event.stopPropagation()
          void form.handleSubmit()
        }}
      >
        <FieldSet>
          <FieldDescription className="text-xs">
            Keep a local history of application errors for troubleshooting. Logs may contain track identifiers and local file paths.
          </FieldDescription>

          <form.Field name="enabled">
            {field => (
              <Field orientation="horizontal">
                <Checkbox
                  id={`${id}-enabled`}
                  name={field.name}
                  checked={field.state.value}
                  onBlur={field.handleBlur}
                  onCheckedChange={(checked) => {
                    field.handleChange(checked === true)
                    void form.handleSubmit()
                  }}
                />
                <FieldLabel htmlFor={`${id}-enabled`}>Enable diagnostic logs</FieldLabel>
              </Field>
            )}
          </form.Field>

          <div>
            <Button type="button" variant="outline" size="sm" onClick={() => setLogsOpen(true)}>
              View logs
              {diagnosticLogCount > 0 ? ` (${diagnosticLogCount})` : ''}
            </Button>
          </div>
        </FieldSet>
      </form>
      <LogsDialog open={logsOpen} onOpenChange={setLogsOpen} />
    </>
  )
}
