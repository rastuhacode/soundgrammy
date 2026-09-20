import { useForm } from '@tanstack/react-form'
import { REGEXP_ONLY_DIGITS } from 'input-otp'
import { Button } from '@/components/ui/button'
import {
  Field,
  FieldDescription,
  FieldError,
  FieldSet,
} from '@/components/ui/fieldset'
import {
  InputOTP,
  InputOTPGroup,
  InputOTPSlot,
} from '@/components/ui/input-otp'
import { codeLoginSchema } from '@/lib/auth/login-schemas'

const CODE_LENGTH = 5

export function CodeLoginStep({
  busy,
  error,
  onValueChange,
  onSubmit,
  onBackToPhone,
}: {
  busy: boolean
  error: string | null
  onValueChange: () => void
  onSubmit: (code: string) => Promise<void>
  onBackToPhone: () => void
}) {
  const form = useForm({
    defaultValues: { code: '' },
    validators: {
      onSubmit: codeLoginSchema,
    },
    onSubmit: async ({ value }) => {
      await onSubmit(value.code)
    },
  })

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        event.stopPropagation()
        void form.handleSubmit()
      }}
      className="flex w-full flex-col gap-3"
      noValidate
    >
      <FieldSet>
        <FieldDescription className="text-center">
          Check the Telegram app chat with &quot;Telegram&quot; for your code.
        </FieldDescription>
        <form.Field name="code">
          {(field) => {
            const validationError = field.state.meta.errors[0]?.message
            const displayError = validationError ?? error
            const invalid = Boolean(displayError)

            return (
              <Field data-invalid={invalid || undefined}>
                <InputOTP
                  id="login-code"
                  name={field.name}
                  maxLength={CODE_LENGTH}
                  value={field.state.value}
                  onChange={(value) => {
                    field.handleChange(value)
                    onValueChange()
                    if (!busy && value.length === CODE_LENGTH) {
                      void form.handleSubmit()
                    }
                  }}
                  onBlur={field.handleBlur}
                  disabled={busy}
                  pattern={REGEXP_ONLY_DIGITS}
                  containerClassName="justify-center"
                  aria-invalid={invalid || undefined}
                  autoComplete="one-time-code"
                  inputMode="numeric"
                  className="w-full"
                >
                  <InputOTPGroup aria-invalid={invalid || undefined} className="w-full">
                    {Array.from({ length: CODE_LENGTH }, (_, index) => (
                      <InputOTPSlot key={index} index={index} className="w-1/5" />
                    ))}
                  </InputOTPGroup>
                </InputOTP>
                <FieldError>{displayError}</FieldError>
              </Field>
            )
          }}
        </form.Field>
      </FieldSet>
      {busy
        ? (
            <p className="text-center text-sm text-muted-foreground">
              Verifying…
            </p>
          )
        : null}
      <Button type="button" variant="outline" onClick={onBackToPhone} disabled={busy}>
        Back to phone number
      </Button>
    </form>
  )
}
