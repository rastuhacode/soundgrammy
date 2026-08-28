import { useForm } from '@tanstack/react-form'
import { z } from 'zod'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Field,
  FieldDescription,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from '@/components/ui/fieldset'
import { Slider } from '@/components/ui/slider'
import { DEFAULT_BOUNCE_SETTINGS } from '@/lib/bounce'
import { useFullscreenStore } from '@/stores/fullscreen-store'

const percentage = z.number().min(0).max(100)
const fullscreenPlayerSchema = z.object({
  keepDisplayAwake: z.boolean(),
  bounceEnabled: z.boolean(),
  strength: percentage,
  balance: percentage,
  smoothness: percentage,
})

export function FullscreenPlayerForm() {
  const bounce = useFullscreenStore(state => state.bounce)
  const keepDisplayAwake = useFullscreenStore(state => state.keepDisplayAwake)
  const setKeepDisplayAwake = useFullscreenStore(state => state.setKeepDisplayAwake)
  const setBounceSettings = useFullscreenStore(state => state.setBounceSettings)
  const resetBounceSettings = useFullscreenStore(state => state.resetBounceSettings)

  const form = useForm({
    defaultValues: {
      keepDisplayAwake,
      bounceEnabled: bounce.enabled,
      strength: bounce.strength,
      balance: bounce.balance,
      smoothness: bounce.smoothness,
    },
    validators: {
      onSubmit: fullscreenPlayerSchema,
    },
    onSubmit: ({ value }) => {
      setKeepDisplayAwake(value.keepDisplayAwake)
      setBounceSettings({
        enabled: value.bounceEnabled,
        strength: value.strength,
        balance: value.balance,
        smoothness: value.smoothness,
      })
    },
  })

  const resetBounce = () => {
    resetBounceSettings()
    form.reset({
      ...form.state.values,
      bounceEnabled: DEFAULT_BOUNCE_SETTINGS.enabled,
      strength: DEFAULT_BOUNCE_SETTINGS.strength,
      balance: DEFAULT_BOUNCE_SETTINGS.balance,
      smoothness: DEFAULT_BOUNCE_SETTINGS.smoothness,
    })
  }

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
            Artwork bounce
          </FieldLegend>
          <FieldDescription className="text-xs">
            Move fullscreen artwork with the track’s overall dynamics and rhythmic accents.
          </FieldDescription>

          <form.Field name="bounceEnabled">
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
                <FieldLabel htmlFor={field.name}>Enable artwork bounce</FieldLabel>
              </Field>
            )}
          </form.Field>

          <form.Subscribe selector={state => state.values.bounceEnabled}>
            {bounceEnabled => (
              <>
                <form.Field name="strength">
                  {field => (
                    <BounceSlider
                      label="Strength"
                      value={field.state.value}
                      disabled={!bounceEnabled}
                      onChange={(value) => {
                        field.handleChange(value)
                        void form.handleSubmit()
                      }}
                    />
                  )}
                </form.Field>
                <form.Field name="balance">
                  {field => (
                    <BounceSlider
                      label="Dynamics ↔ Beats"
                      value={field.state.value}
                      disabled={!bounceEnabled}
                      onChange={(value) => {
                        field.handleChange(value)
                        void form.handleSubmit()
                      }}
                      left="Dynamics"
                      right="Beats"
                    />
                  )}
                </form.Field>
                <form.Field name="smoothness">
                  {field => (
                    <BounceSlider
                      label="Smoothness"
                      value={field.state.value}
                      disabled={!bounceEnabled}
                      onChange={(value) => {
                        field.handleChange(value)
                        void form.handleSubmit()
                      }}
                      left="Snappy"
                      right="Fluid"
                    />
                  )}
                </form.Field>
              </>
            )}
          </form.Subscribe>

          <div>
            <Button type="button" variant="outline" size="sm" onClick={resetBounce}>
              Reset defaults
            </Button>
          </div>
        </FieldSet>
      </FieldSet>
    </form>
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
    <Field className="gap-2">
      <div className="flex items-center justify-between gap-3 text-sm">
        <FieldLabel>{props.label}</FieldLabel>
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
    </Field>
  )
}
