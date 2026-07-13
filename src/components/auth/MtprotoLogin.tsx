import { useCallback, useEffect, useRef, useState } from 'react'
import { toDataURL } from 'qrcode'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api'
import type { AuthUser } from '@/types'

type Step = 'qr' | 'phone' | 'code' | 'password' | 'qr-password'

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback
}

export function MtprotoLogin({
  onAuthenticated,
}: {
  onAuthenticated: (user: AuthUser) => void
}) {
  const [step, setStep] = useState<Step>('qr')
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null)
  const [qrStarting, setQrStarting] = useState(false)
  const [phoneNumber, setPhoneNumber] = useState('')
  const [code, setCode] = useState('')
  const [password, setPassword] = useState('')
  const [passwordHint, setPasswordHint] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const lastUrlRef = useRef<string | null>(null)

  const renderQr = useCallback(async (url: string) => {
    if (lastUrlRef.current === url && qrDataUrl) return
    lastUrlRef.current = url
    try {
      const dataUrl = await toDataURL(url, {
        width: 232,
        margin: 1,
        color: { dark: '#000000', light: '#ffffff' },
      })
      setQrDataUrl(dataUrl)
    }
    catch {
      setQrDataUrl(null)
    }
  }, [qrDataUrl])

  const startQrLogin = useCallback(async () => {
    setError(null)
    setQrStarting(true)
    setQrDataUrl(null)
    lastUrlRef.current = null
    try {
      const outcome = await api.qrStart()
      if (outcome.status === 'waiting') {
        await renderQr(outcome.url)
        setStep('qr')
      }
      else if (outcome.status === 'passwordRequired') {
        setPasswordHint(outcome.hint)
        setStep('qr-password')
      }
      else if (outcome.status === 'authorized') {
        onAuthenticated(outcome.user)
      }
    }
    catch (err) {
      setError(errorMessage(err, 'Failed to start QR login'))
    }
    finally {
      setQrStarting(false)
    }
  }, [renderQr, onAuthenticated])

  useEffect(() => {
    // QR login must be kicked off on mount; the state writes inside startQrLogin
    // reflect async MTProto progress, not derived render state.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    startQrLogin()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Poll the QR token while the QR step is visible.
  useEffect(() => {
    if (step !== 'qr') return
    let cancelled = false

    const interval = setInterval(async () => {
      if (cancelled) return
      try {
        const outcome = await api.qrPoll()
        if (cancelled) return
        if (outcome.status === 'waiting') {
          await renderQr(outcome.url)
        }
        else if (outcome.status === 'passwordRequired') {
          setPasswordHint(outcome.hint)
          setStep('qr-password')
        }
        else if (outcome.status === 'authorized') {
          onAuthenticated(outcome.user)
        }
      }
      catch {
        // transient errors while polling are ignored
      }
    }, 2000)

    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [step, renderQr, onAuthenticated])

  const handleSendCode = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setBusy(true)
    try {
      await api.phoneSendCode(phoneNumber)
      setStep('code')
    }
    catch (err) {
      setError(errorMessage(err, 'Failed to send code'))
    }
    finally {
      setBusy(false)
    }
  }

  const handleSignIn = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setBusy(true)
    try {
      const outcome = await api.phoneSignIn(code)
      if (outcome.status === 'passwordRequired') {
        setPasswordHint(outcome.hint)
        setStep('password')
        return
      }
      onAuthenticated(outcome.user)
    }
    catch (err) {
      setError(errorMessage(err, 'Failed to sign in'))
    }
    finally {
      setBusy(false)
    }
  }

  const handlePhonePassword = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setBusy(true)
    try {
      const user = await api.phoneCheckPassword(password)
      onAuthenticated(user)
    }
    catch (err) {
      setError(errorMessage(err, 'Invalid password'))
    }
    finally {
      setBusy(false)
    }
  }

  const handleQrPassword = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setBusy(true)
    try {
      const user = await api.qrCheckPassword(password)
      onAuthenticated(user)
    }
    catch (err) {
      setError(errorMessage(err, 'Invalid password'))
    }
    finally {
      setBusy(false)
    }
  }

  return (
    <div className="hifi-bg relative flex min-h-screen items-center justify-center overflow-hidden px-5 py-12">
      <div className="pointer-events-none absolute left-1/2 top-[-10%] h-144 w-xl -translate-x-1/2 rounded-full bg-primary/15 blur-[120px] animate-glow-pulse" />

      <div className="relative w-full max-w-104 animate-fade-up">
        <div className="mb-8 text-center">
          <span
            aria-hidden
            className="equalizer mx-auto mb-5 h-7 justify-center [&>span]:w-1"
          >
            <span />
            <span />
            <span />
            <span />
          </span>
          <h1 className="text-4xl font-semibold tracking-tight text-foreground text-glow">
            Sound
            <span className="text-primary">grammy</span>
          </h1>
          <p className="mx-auto mt-3 max-w-xs text-sm leading-relaxed text-muted-foreground">
            Sign in with Telegram to tune in your profile music.
          </p>
        </div>

        <div className="rounded-2xl border border-border bg-card/80 p-7 shadow-[0_24px_60px_-24px_rgba(0,0,0,0.7)] backdrop-blur-xl">
          {error
            ? (
                <p
                  className="mb-5 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-center text-sm text-destructive"
                  role="alert"
                >
                  {error}
                </p>
              )
            : null}

          {step === 'qr' || step === 'qr-password'
            ? (
                <div className="flex w-full flex-col items-center gap-4">
                  {step === 'qr'
                    ? (
                        <>
                          <p className="text-center text-sm leading-relaxed text-muted-foreground">
                            Scan with Telegram on your phone
                            <br />
                            <span className="font-mono text-xs uppercase tracking-wide text-foreground/70">
                              Settings → Devices → Link Desktop
                            </span>
                          </p>
                          <div className="relative rounded-xl border border-border bg-white p-3">
                            {qrDataUrl
                              ? (
                                  <img
                                    src={qrDataUrl}
                                    alt="Telegram login QR code"
                                    width={232}
                                    height={232}
                                    className="rounded-md"
                                  />
                                )
                              : (
                                  <div className="flex h-[232px] w-[232px] items-center justify-center">
                                    <span className="font-mono text-xs text-black/60">
                                      {qrStarting ? 'Generating…' : 'Loading…'}
                                    </span>
                                  </div>
                                )}
                          </div>
                          <p className="text-center text-xs text-muted-foreground/80">
                            Works without SMS — recommended for Russian numbers
                          </p>
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={startQrLogin}
                            disabled={qrStarting}
                          >
                            Refresh QR code
                          </Button>
                        </>
                      )
                    : (
                        <form
                          onSubmit={e => handleQrPassword(e)}
                          className="flex w-full flex-col gap-3"
                        >
                          <p className="text-center text-sm text-muted-foreground">
                            2FA enabled
                            {passwordHint ? `: ${passwordHint}` : ''}
                          </p>
                          <Input
                            type="password"
                            placeholder="2FA password"
                            value={password}
                            onChange={e => setPassword(e.target.value)}
                            required
                          />
                          <Button type="submit" disabled={busy} className="w-full">
                            {busy ? 'Signing in…' : 'Continue'}
                          </Button>
                        </form>
                      )}
                  <div className="h-px w-full bg-border" />
                  <Button
                    type="button"
                    variant="outline"
                    className="w-full"
                    onClick={() => setStep('phone')}
                  >
                    Use phone number instead
                  </Button>
                </div>
              )
            : step === 'phone'
              ? (
                  <form
                    onSubmit={e => handleSendCode(e)}
                    className="flex w-full flex-col gap-3"
                  >
                    <p className="text-center text-xs leading-relaxed text-muted-foreground">
                      Phone login may not work for Russian numbers (SMS blocked). Prefer
                      QR login.
                    </p>
                    <Input
                      type="tel"
                      placeholder="+79991234567"
                      value={phoneNumber}
                      onChange={e => setPhoneNumber(e.target.value)}
                      required
                    />
                    <Button type="submit" disabled={busy} className="w-full">
                      {busy ? 'Sending…' : 'Send login code'}
                    </Button>
                    <Button type="button" variant="ghost" onClick={() => setStep('qr')}>
                      Back to QR login
                    </Button>
                  </form>
                )
              : step === 'code'
                ? (
                    <form
                      onSubmit={e => handleSignIn(e)}
                      className="flex w-full flex-col gap-3"
                    >
                      <p className="text-center text-sm leading-relaxed text-muted-foreground">
                        Check the Telegram app chat with &quot;Telegram&quot; for your
                        code.
                      </p>
                      <Input
                        type="text"
                        inputMode="numeric"
                        placeholder="Login code"
                        value={code}
                        onChange={e => setCode(e.target.value)}
                        required
                        className="text-center font-mono text-lg tracking-[0.5em]"
                      />
                      <Button type="submit" disabled={busy} className="w-full">
                        {busy ? 'Verifying…' : 'Sign in'}
                      </Button>
                      <Button type="button" variant="ghost" onClick={() => setStep('qr')}>
                        Back to QR login
                      </Button>
                    </form>
                  )
                : (
                    <form
                      onSubmit={e => handlePhonePassword(e)}
                      className="flex w-full flex-col gap-3"
                    >
                      <p className="text-center text-sm text-muted-foreground">
                        2FA enabled
                        {passwordHint ? `: ${passwordHint}` : ''}
                      </p>
                      <Input
                        type="password"
                        placeholder="2FA password"
                        value={password}
                        onChange={e => setPassword(e.target.value)}
                        required
                      />
                      <Button type="submit" disabled={busy} className="w-full">
                        {busy ? 'Signing in…' : 'Sign in'}
                      </Button>
                    </form>
                  )}
        </div>

        <p className="mt-6 text-center font-mono text-[10px] uppercase tracking-[0.3em] text-muted-foreground/60">
          End-to-end via MTProto
        </p>
      </div>
    </div>
  )
}
