import { useEffect, useRef, useState } from 'react'
import { MaskInput } from 'maska'
import { Button } from '@/components/ui/button'
import { Field, FieldError, FieldSet } from '@/components/ui/fieldset'
import { Input } from '@/components/ui/input'
import { firstIssueMessage, phoneLoginSchema } from '@/lib/auth/login-schemas'

/** Pick a readable mask from digit count so +7… stays `+7 (###) ###-##-##`. */
function phoneMaskFor(value: string): string {
  const digits = value.replace(/\D/g, '')
  if (digits.length <= 11) return '+# (###) ###-##-##'
  if (digits.length <= 12) return '+## ### ### ###'
  if (digits.length <= 13) return '+### ## ### ####'
  return '+#### ### ## ## ##'
}

function toE164(unmaskedDigits: string): string {
  if (!unmaskedDigits) return ''
  return `+${unmaskedDigits}`
}

export function PhoneLoginStep({
  phoneNumber,
  busy,
  error,
  onPhoneNumberChange,
  onSubmit,
  onBackToQr,
}: {
  phoneNumber: string
  busy: boolean
  error: string | null
  onPhoneNumberChange: (value: string) => void
  onSubmit: (phoneNumber: string) => void
  onBackToQr: () => void
}) {
  const [fieldError, setFieldError] = useState<string | null>(null)
  const displayError = fieldError ?? error
  const invalid = Boolean(displayError)

  const inputRef = useRef<HTMLInputElement>(null)
  const onPhoneNumberChangeRef = useRef(onPhoneNumberChange)
  const setFieldErrorRef = useRef(setFieldError)

  useEffect(() => {
    onPhoneNumberChangeRef.current = onPhoneNumberChange
  }, [onPhoneNumberChange])

  useEffect(() => {
    const input = inputRef.current
    if (!input) return

    const mask = new MaskInput(input, {
      mask: phoneMaskFor,
      eager: true,
      preProcess: (value) => {
        // Keep a single leading + and digits only.
        const cleaned = value.replace(/[^\d+]/g, '')
        const digits = cleaned.replace(/\D/g, '')
        return digits.length > 0 ? `+${digits}` : ''
      },
      onMaska: (detail) => {
        setFieldErrorRef.current(null)
        onPhoneNumberChangeRef.current(toE164(detail.unmasked))
      },
    })

    if (phoneNumber) {
      input.value = phoneNumber
      mask.updateValue(input)
    }

    return () => mask.destroy()
    // Attach once; mask owns the input value after mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const result = phoneLoginSchema.safeParse({ phoneNumber })
    if (!result.success) {
      setFieldError(firstIssueMessage(result.error))
      return
    }
    setFieldError(null)
    onSubmit(result.data.phoneNumber)
  }

  return (
    <form onSubmit={handleSubmit} className="mx-auto flex w-full flex-col gap-3" noValidate>
      <FieldSet>
        <Field data-invalid={invalid || undefined}>
          <Input
            ref={inputRef}
            id="login-phone"
            type="tel"
            inputMode="tel"
            placeholder="+7 (999) 123-45-67"
            aria-invalid={invalid || undefined}
            autoComplete="tel"
            disabled={busy}
          />
          <FieldError>{displayError}</FieldError>
        </Field>
      </FieldSet>
      <Button type="submit" disabled={busy}>
        {busy ? 'Sending…' : 'Send login code'}
      </Button>
      <Button type="button" variant="outline" onClick={onBackToQr} disabled={busy}>
        Use QR instead
      </Button>
    </form>
  )
}
