import { useForm } from '@tanstack/react-form'
import { Button } from '@/components/ui/button'
import { Field, FieldDescription, FieldError, FieldSet } from '@/components/ui/fieldset'
import { Input } from '@/components/ui/input'
import { passwordLoginSchema } from '@/lib/auth/login-schemas'

export function PasswordLoginStep({
  passwordHint,
  busy,
  error,
  submitLabel,
  showBackToQr,
  onValueChange,
  onSubmit,
  onBackToQr,
  onUsePhone,
}: {
  passwordHint: string | null
  busy: boolean
  error: string | null
  submitLabel: string
  showBackToQr: boolean
  onValueChange: () => void
  onSubmit: (password: string) => Promise<void>
  onBackToQr?: () => void
  onUsePhone?: () => void
}) {
  const form = useForm({
    defaultValues: { password: '' },
    validators: {
      onSubmit: passwordLoginSchema,
    },
    onSubmit: async ({ value }) => {
      await onSubmit(value.password)
    },
  })

  return (
    <div className="flex w-full flex-col items-center gap-4">
      <form
        onSubmit={(event) => {
          event.preventDefault()
          event.stopPropagation()
          void form.handleSubmit()
        }}
        className="flex w-full flex-col gap-3"
        noValidate
      >
        <FieldSet disabled={busy}>
          {passwordHint
            ? (
                <FieldDescription className="text-center">
                  Password hint:
                  {' '}
                  {passwordHint}
                </FieldDescription>
              )
            : null}
          <form.Field name="password">
            {(field) => {
              const validationError = field.state.meta.errors[0]?.message
              const displayError = validationError ?? error
              const invalid = Boolean(displayError)

              return (
                <Field data-invalid={invalid || undefined}>
                  <Input
                    id="login-password"
                    name={field.name}
                    type="password"
                    placeholder="2FA password"
                    value={field.state.value}
                    onChange={(event) => {
                      field.handleChange(event.target.value)
                      onValueChange()
                    }}
                    onBlur={field.handleBlur}
                    aria-invalid={invalid || undefined}
                    autoComplete="current-password"
                  />
                  <FieldError>{displayError}</FieldError>
                </Field>
              )
            }}
          </form.Field>
        </FieldSet>
        <Button type="submit" disabled={busy} className="w-full">
          {busy ? 'Signing in…' : submitLabel}
        </Button>
        {showBackToQr && onBackToQr
          ? (
              <Button
                type="button"
                variant="outline"
                onClick={onBackToQr}
                disabled={busy}
              >
                Back to QR login
              </Button>
            )
          : null}
      </form>
      {onUsePhone
        ? (
            <Button
              type="button"
              variant="outline"
              className="w-full"
              onClick={onUsePhone}
              disabled={busy}
            >
              Back to phone number
            </Button>
          )
        : null}
    </div>
  )
}
