import { useCallback, useEffect, useRef, useState } from 'react'
import { toDataURL } from 'qrcode'
import { api } from '@/lib/api'
import type { AuthUser, QrOutcome } from '@/types'

export type LoginStep = 'qr' | 'phone' | 'code' | 'password' | 'qr-password'

const POLL_INTERVAL_MS = 2000
const MAX_CONSECUTIVE_POLL_FAILURES = 3
/** Refresh when the token is within this many seconds of expiry. */
const EXPIRY_SLACK_SECONDS = 5

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback
}

function isExpiredOrNear(expires: number, nowSeconds = Date.now() / 1000): boolean {
  return expires <= nowSeconds + EXPIRY_SLACK_SECONDS
}

export function useMtprotoLogin(onAuthenticated: (user: AuthUser) => void) {
  const [step, setStep] = useState<LoginStep>('qr')
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null)
  const [qrLoading, setQrLoading] = useState(false)
  const [phoneNumber, setPhoneNumber] = useState('')
  const [code, setCode] = useState('')
  const [password, setPassword] = useState('')
  const [passwordHint, setPasswordHint] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const lastUrlRef = useRef<string | null>(null)
  const qrExpiresRef = useRef<number | null>(null)
  const pollFailuresRef = useRef(0)
  const generationRef = useRef(0)
  /** Phone that already has a pending login code on the backend. */
  const codeSentForPhoneRef = useRef<string | null>(null)
  const onAuthenticatedRef = useRef(onAuthenticated)

  useEffect(() => {
    onAuthenticatedRef.current = onAuthenticated
  }, [onAuthenticated])

  const renderQr = useCallback(async (url: string) => {
    if (lastUrlRef.current === url) return
    lastUrlRef.current = url
    try {
      const dataUrl = await toDataURL(url, {
        width: 232,
        margin: 1,
        color: { dark: '#000000', light: '#ffffff' },
      })
      if (lastUrlRef.current !== url) return
      setQrDataUrl(dataUrl)
    }
    catch {
      if (lastUrlRef.current === url) setQrDataUrl(null)
    }
  }, [])

  const applyQrOutcome = useCallback(async (
    outcome: QrOutcome,
    generation: number,
  ): Promise<'continue' | 'done'> => {
    if (generation !== generationRef.current) return 'done'

    if (outcome.status === 'waiting') {
      qrExpiresRef.current = outcome.expires
      if (isExpiredOrNear(outcome.expires)) {
        return 'continue'
      }
      await renderQr(outcome.url)
      if (generation !== generationRef.current) return 'done'
      setStep('qr')
      return 'continue'
    }

    if (outcome.status === 'passwordRequired') {
      setPassword('')
      setPasswordHint(outcome.hint)
      setStep('qr-password')
      return 'done'
    }

    onAuthenticatedRef.current(outcome.user)
    return 'done'
  }, [renderQr])

  const startQrLogin = useCallback(async () => {
    const generation = ++generationRef.current
    setError(null)
    setQrLoading(true)
    setQrDataUrl(null)
    lastUrlRef.current = null
    qrExpiresRef.current = null
    pollFailuresRef.current = 0
    codeSentForPhoneRef.current = null
    setStep('qr')

    try {
      const outcome = await api.qrStart()
      if (generation !== generationRef.current) return

      if (outcome.status === 'waiting' && isExpiredOrNear(outcome.expires)) {
        // Token already stale — try once more immediately.
        const retry = await api.qrStart()
        if (generation !== generationRef.current) return
        await applyQrOutcome(retry, generation)
        return
      }

      await applyQrOutcome(outcome, generation)
    }
    catch (err) {
      if (generation !== generationRef.current) return
      setError(errorMessage(err, 'Failed to start QR login'))
    }
    finally {
      if (generation === generationRef.current) {
        setQrLoading(false)
      }
    }
  }, [applyQrOutcome])

  useEffect(() => {
    // Kick off QR on mount; async progress writes live auth state, not derived render state.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    startQrLogin()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Poll while the QR step is visible. Deps intentionally exclude renderQr churn.
  useEffect(() => {
    if (step !== 'qr') return
    let cancelled = false

    const tick = async () => {
      if (cancelled) return
      const generation = generationRef.current

      const expires = qrExpiresRef.current
      if (expires !== null && isExpiredOrNear(expires)) {
        await startQrLogin()
        return
      }

      try {
        const outcome = await api.qrPoll()
        if (cancelled || generation !== generationRef.current) return

        if (pollFailuresRef.current > 0) {
          pollFailuresRef.current = 0
          setError(null)
        }

        if (outcome.status === 'waiting' && isExpiredOrNear(outcome.expires)) {
          await startQrLogin()
          return
        }

        await applyQrOutcome(outcome, generation)
      }
      catch {
        if (cancelled || generation !== generationRef.current) return
        pollFailuresRef.current += 1
        if (pollFailuresRef.current >= MAX_CONSECUTIVE_POLL_FAILURES) {
          setError('QR login is unreachable. Check your connection or refresh the code.')
        }
      }
    }

    const interval = setInterval(tick, POLL_INTERVAL_MS)

    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [step, applyQrOutcome, startQrLogin])

  const goToPhone = useCallback(() => {
    generationRef.current += 1
    setError(null)
    setPassword('')
    setCode('')
    setStep('phone')
  }, [])

  /** Leave QR (or QR 2FA) for phone — drop any prior phone-code claim. */
  const switchToPhone = useCallback(() => {
    codeSentForPhoneRef.current = null
    goToPhone()
  }, [goToPhone])

  const goToQr = useCallback(() => {
    setPassword('')
    setCode('')
    startQrLogin()
  }, [startQrLogin])

  const setPhoneNumberValue = useCallback((value: string) => {
    setError(null)
    if (codeSentForPhoneRef.current && codeSentForPhoneRef.current !== value.trim()) {
      codeSentForPhoneRef.current = null
    }
    setPhoneNumber(value)
  }, [])

  const setCodeValue = useCallback((value: string) => {
    setError(null)
    setCode(value)
  }, [])

  const setPasswordValue = useCallback((value: string) => {
    setError(null)
    setPassword(value)
  }, [])

  const handleSendCode = useCallback(async (phone: string) => {
    setError(null)
    setPhoneNumber(phone)

    // Returning from the code step with the same number: reuse the pending token
    // instead of calling sendCode again (Telegram often rejects that first attempt).
    if (codeSentForPhoneRef.current === phone) {
      setCode('')
      setStep('code')
      return
    }

    setBusy(true)
    try {
      try {
        await api.phoneSendCode(phone)
      }
      catch {
        // After an interrupted QR attempt, Telegram often rejects the first
        // sendCode; one immediate retry usually succeeds.
        await api.phoneSendCode(phone)
      }
      codeSentForPhoneRef.current = phone
      setCode('')
      setStep('code')
    }
    catch (err) {
      setError(errorMessage(err, 'Failed to send code'))
    }
    finally {
      setBusy(false)
    }
  }, [])

  const handleSignIn = useCallback(async (loginCode: string) => {
    setError(null)
    setCode(loginCode)
    setBusy(true)
    try {
      const outcome = await api.phoneSignIn(loginCode)
      if (outcome.status === 'passwordRequired') {
        // Phone token was consumed; a later "send code" must hit the API again.
        codeSentForPhoneRef.current = null
        setPassword('')
        setPasswordHint(outcome.hint)
        setStep('password')
        return
      }
      codeSentForPhoneRef.current = null
      onAuthenticatedRef.current(outcome.user)
    }
    catch (err) {
      setError(errorMessage(err, 'Failed to sign in'))
    }
    finally {
      setBusy(false)
    }
  }, [])

  const handlePassword = useCallback(async (loginPassword: string) => {
    setError(null)
    setPassword(loginPassword)
    setBusy(true)
    try {
      const user = step === 'qr-password'
        ? await api.qrCheckPassword(loginPassword)
        : await api.phoneCheckPassword(loginPassword)
      onAuthenticatedRef.current(user)
    }
    catch (err) {
      setError(errorMessage(err, 'Invalid password'))
    }
    finally {
      setBusy(false)
    }
  }, [step])

  return {
    step,
    qrDataUrl,
    qrLoading,
    phoneNumber,
    setPhoneNumber: setPhoneNumberValue,
    code,
    setCode: setCodeValue,
    password,
    setPassword: setPasswordValue,
    passwordHint,
    error,
    busy,
    startQrLogin,
    goToPhone,
    switchToPhone,
    goToQr,
    handleSendCode,
    handleSignIn,
    handlePassword,
  }
}
