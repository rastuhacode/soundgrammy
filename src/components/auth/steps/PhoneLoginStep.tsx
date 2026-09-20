import { useEffect, useRef } from 'react'
import { useForm } from '@tanstack/react-form'
import { MaskInput } from 'maska'
import { Button } from '@/components/ui/button'
import { Field, FieldError, FieldSet } from '@/components/ui/fieldset'
import { Input } from '@/components/ui/input'
import { phoneLoginSchema } from '@/lib/auth/login-schemas'

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
  onSubmit: (phoneNumber: string) => Promise<void>
  onBackToQr: () => void
}) {
  const inputRef = useRef<HTMLInputElement>(null)
  const onPhoneNumberChangeRef = useRef(onPhoneNumberChange)

  const form = useForm({
    defaultValues: { phoneNumber },
    validators: {
      onSubmit: phoneLoginSchema,
    },
    onSubmit: async ({ value }) => {
      await onSubmit(value.phoneNumber)
    },
  })

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
        const nextPhoneNumber = toE164(detail.unmasked)
        form.setFieldValue('phoneNumber', nextPhoneNumber)
        onPhoneNumberChangeRef.current(nextPhoneNumber)
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

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        event.stopPropagation()
        void form.handleSubmit()
      }}
      className="mx-auto flex w-full flex-col gap-3"
      noValidate
    >
      <FieldSet>
        <form.Field name="phoneNumber">
          {(field) => {
            const validationError = field.state.meta.errors[0]?.message
            const displayError = validationError ?? error
            const invalid = Boolean(displayError)

            return (
              <Field data-invalid={invalid || undefined}>
                <Input
                  ref={inputRef}
                  id="login-phone"
                  name={field.name}
                  type="tel"
                  inputMode="tel"
                  placeholder="+7 (999) 123-45-67"
                  aria-invalid={invalid || undefined}
                  autoComplete="tel"
                  disabled={busy}
                  onBlur={field.handleBlur}
                />
                <FieldError>{displayError}</FieldError>
              </Field>
            )
          }}
        </form.Field>
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
