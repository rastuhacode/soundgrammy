import { useForm } from '@tanstack/react-form'
import { z } from 'zod'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Field,
  FieldDescription,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from '@/components/ui/fieldset'
import { Switch } from '@/components/ui/switch'
import { useFullscreenStore } from '@/stores/fullscreen-store'

const fullscreenPlayerSchema = z.object({
  keepDisplayAwake: z.boolean(),
  bounceEnabled: z.boolean(),
})

export function FullscreenPlayerForm() {
  const bounce = useFullscreenStore(state => state.bounce)
  const keepDisplayAwake = useFullscreenStore(state => state.keepDisplayAwake)
  const setKeepDisplayAwake = useFullscreenStore(state => state.setKeepDisplayAwake)
  const setBounceEnabled = useFullscreenStore(state => state.setBounceEnabled)

  const form = useForm({
    defaultValues: {
      keepDisplayAwake,
      bounceEnabled: bounce.enabled,
    },
    validators: {
      onSubmit: fullscreenPlayerSchema,
    },
    onSubmit: ({ value }) => {
      setKeepDisplayAwake(value.keepDisplayAwake)
      setBounceEnabled(value.bounceEnabled)
    },
  })

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        event.stopPropagation()
        void form.handleSubmit()
      }}
    >
      <FieldSet>
        <FieldDescription className="text-xs">
          Prevent normal inactivity from dimming or turning off the display while the fullscreen player is open. This may increase battery usage.
        </FieldDescription>

        <form.Field name="keepDisplayAwake">
          {field => (
            <Field orientation="horizontal">
              <Checkbox
                id={field.name}
                name={field.name}
                checked={field.state.value}
                onBlur={field.handleBlur}
                onCheckedChange={(checked) => {
                  field.handleChange(checked === true)
                  void form.handleSubmit()
                }}
              />
              <FieldLabel htmlFor={field.name}>Keep display awake</FieldLabel>
            </Field>
          )}
        </form.Field>

        <FieldSet className="border-t border-border/60 pt-4">
          <FieldLegend className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Dynamic visuals
          </FieldLegend>
          <FieldDescription className="text-xs">
            React to the music with artwork movement and colors: bass below, midrange in the center, and high frequencies above. Intensity adapts automatically.
          </FieldDescription>

          <form.Field name="bounceEnabled">
            {field => (
              <Field orientation="horizontal">
                <Switch
                  id={field.name}
                  name={field.name}
                  checked={field.state.value}
                  onBlur={field.handleBlur}
                  onCheckedChange={(checked) => {
                    field.handleChange(checked)
                    void form.handleSubmit()
                  }}
                />
                <FieldLabel htmlFor={field.name}>Enable dynamic visuals</FieldLabel>
              </Field>
            )}
          </form.Field>

        </FieldSet>
      </FieldSet>
    </form>
  )
}
