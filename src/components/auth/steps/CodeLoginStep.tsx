import { useState } from 'react'
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
import { codeLoginSchema, firstIssueMessage } from '@/lib/auth/login-schemas'

const CODE_LENGTH = 5

export function CodeLoginStep({
  code,
  busy,
  error,
  onCodeChange,
  onSubmit,
  onBackToPhone,
}: {
  code: string
  busy: boolean
  error: string | null
  onCodeChange: (value: string) => void
  onSubmit: (code: string) => void
  onBackToPhone: () => void
}) {
  const [fieldError, setFieldError] = useState<string | null>(null)
  const displayError = fieldError ?? error
  const invalid = Boolean(displayError)

  const trySubmit = (value: string) => {
    if (busy || value.length !== CODE_LENGTH) return
    const result = codeLoginSchema.safeParse({ code: value })
    if (!result.success) {
      setFieldError(firstIssueMessage(result.error))
      return
    }
    setFieldError(null)
    onSubmit(result.data.code)
  }

  return (
    <div className="flex w-full flex-col gap-3">
      <FieldSet>
        <FieldDescription className="text-center">
          Check the Telegram app chat with &quot;Telegram&quot; for your code.
        </FieldDescription>
        <Field data-invalid={invalid || undefined}>
          <InputOTP
            id="login-code"
            maxLength={CODE_LENGTH}
            value={code}
            onChange={(value) => {
              setFieldError(null)
              onCodeChange(value)
              trySubmit(value)
            }}
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
    </div>
  )
}
