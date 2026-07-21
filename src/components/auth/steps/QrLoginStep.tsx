import { Button } from '@/components/ui/button'
import {
  Field,
  FieldDescription,
  FieldError,
  FieldSet,
} from '@/components/ui/fieldset'

export function QrLoginStep({
  qrDataUrl,
  qrLoading,
  busy,
  error,
  onRefresh,
  onUsePhone,
}: {
  qrDataUrl: string | null
  qrLoading: boolean
  busy: boolean
  error: string | null
  onRefresh: () => void
  onUsePhone: () => void
}) {
  const invalid = Boolean(error)

  return (
    <div className="flex w-full flex-col items-center gap-4">
      <FieldSet className="w-full items-center gap-4">
        <FieldDescription className="text-center">
          Scan with Telegram on your phone
          <br />
          <span className="font-mono text-xs uppercase tracking-wide text-foreground/70">
            Settings → Devices → Link Desktop
          </span>
        </FieldDescription>

        <Field data-invalid={invalid || undefined} className="items-center w-fit">
          <div className="relative rounded-xl border border-border bg-white p-3">
            {qrDataUrl
              ? (
                  <img src={qrDataUrl} alt="Telegram login QR code" className="size-56 rounded-md" />
                )
              : (
                  <div className="flex size-56 items-center justify-center">
                    <span className="font-mono text-xs text-black/60">
                      {qrLoading ? 'Generating…' : 'Loading…'}
                    </span>
                  </div>
                )}
          </div>
          <FieldError className="text-center">{error}</FieldError>
        </Field>
      </FieldSet>

      {error
        ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={onRefresh}
              disabled={qrLoading || busy}
            >
              Refresh QR code
            </Button>
          )
        : null}

      <Button
        type="button"
        variant="outline"
        onClick={onUsePhone}
        disabled={busy}
        className="w-full"
      >
        Use phone number instead
      </Button>
    </div>
  )
}
