import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Field, FieldError, FieldSet } from '@/components/ui/fieldset'
import { Input } from '@/components/ui/input'
import { firstIssueMessage, passwordLoginSchema } from '@/lib/auth/login-schemas'

export function PasswordLoginStep({
  password,
  busy,
  error,
  submitLabel,
  showBackToQr,
  onPasswordChange,
  onSubmit,
  onBackToQr,
  onUsePhone,
}: {
  password: string
  passwordHint: string | null
  busy: boolean
  error: string | null
  submitLabel: string
  showBackToQr: boolean
  onPasswordChange: (value: string) => void
  onSubmit: (password: string) => void
  onBackToQr?: () => void
  onUsePhone?: () => void
}) {
  const [fieldError, setFieldError] = useState<string | null>(null)
  const displayError = fieldError ?? error
  const invalid = Boolean(displayError)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const result = passwordLoginSchema.safeParse({ password })
    if (!result.success) {
      setFieldError(firstIssueMessage(result.error))
      return
    }
    setFieldError(null)
    onSubmit(result.data.password)
  }

  return (
    <div className="flex w-full flex-col items-center gap-4">
      <form
        onSubmit={handleSubmit}
        className="flex w-full flex-col gap-3"
        noValidate
      >
        <FieldSet>
          <Field data-invalid={invalid || undefined}>
            <Input
              id="login-password"
              type="password"
              placeholder="2FA password"
              value={password}
              onChange={(e) => {
                setFieldError(null)
                onPasswordChange(e.target.value)
              }}
              aria-invalid={invalid || undefined}
              autoComplete="current-password"
            />
            <FieldError>{displayError}</FieldError>
          </Field>
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
