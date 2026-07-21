import type { ReactNode } from 'react'
import { LoginShell } from '@/components/auth/LoginShell'
import { CodeLoginStep } from '@/components/auth/steps/CodeLoginStep'
import { PasswordLoginStep } from '@/components/auth/steps/PasswordLoginStep'
import { PhoneLoginStep } from '@/components/auth/steps/PhoneLoginStep'
import { QrLoginStep } from '@/components/auth/steps/QrLoginStep'
import { useMtprotoLogin } from '@/hooks/auth/use-mtproto-login'
import type { AuthUser } from '@/types'

export function MtprotoLogin({
  onAuthenticated,
}: {
  onAuthenticated: (user: AuthUser) => void
}) {
  const login = useMtprotoLogin(onAuthenticated)

  let body: ReactNode
  switch (login.step) {
    case 'qr':
      body = (
        <QrLoginStep
          qrDataUrl={login.qrDataUrl}
          qrLoading={login.qrLoading}
          busy={login.busy}
          error={login.error}
          onRefresh={login.startQrLogin}
          onUsePhone={login.switchToPhone}
        />
      )
      break
    case 'qr-password':
      body = (
        <PasswordLoginStep
          password={login.password}
          passwordHint={login.passwordHint}
          busy={login.busy}
          error={login.error}
          submitLabel="Continue"
          showBackToQr
          onPasswordChange={login.setPassword}
          onSubmit={password => login.handlePassword(password)}
          onBackToQr={login.goToQr}
          onUsePhone={login.switchToPhone}
        />
      )
      break
    case 'phone':
      body = (
        <PhoneLoginStep
          phoneNumber={login.phoneNumber}
          busy={login.busy}
          error={login.error}
          onPhoneNumberChange={login.setPhoneNumber}
          onSubmit={phone => login.handleSendCode(phone)}
          onBackToQr={login.goToQr}
        />
      )
      break
    case 'code':
      body = (
        <CodeLoginStep
          code={login.code}
          busy={login.busy}
          error={login.error}
          onCodeChange={login.setCode}
          onSubmit={code => login.handleSignIn(code)}
          onBackToPhone={login.goToPhone}
        />
      )
      break
    case 'password':
      body = (
        <PasswordLoginStep
          password={login.password}
          passwordHint={login.passwordHint}
          busy={login.busy}
          error={login.error}
          submitLabel="Sign in"
          showBackToQr={false}
          onPasswordChange={login.setPassword}
          onSubmit={password => login.handlePassword(password)}
          onUsePhone={login.goToPhone}
        />
      )
      break
  }

  return (
    <LoginShell>
      <div className="w-75 mx-auto">
        {body}
      </div>
    </LoginShell>
  )
}
