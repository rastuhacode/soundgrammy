import { useEffect, useState, type ReactNode } from 'react'
import { ChevronDown } from 'lucide-react'
import { api } from '@/lib/api'
import { ProxySettingsForm } from '@/components/settings/forms/ProxySettingsForm'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'

export function LoginShell({ children }: { children: ReactNode }) {
  const [proxyOpen, setProxyOpen] = useState(false)
  const [proxyHint, setProxyHint] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const view = await api.getProxySettings()
        if (cancelled) return
        if (view.applyError) {
          setProxyOpen(true)
          setProxyHint(
            view.enabled
              ? 'Could not reach Telegram via proxy. Check tg-ws-proxy is running, then Apply again.'
              : 'Telegram is unreachable. Turn on “Use MTProto proxy”, paste your tg://proxy link, then Apply & reconnect.',
          )
        }
        else if (!view.telegramOnline) {
          setProxyOpen(true)
          setProxyHint(
            'Not connected to Telegram. Enable MTProto proxy (or VPN) and Apply & reconnect.',
          )
        }
        else if (view.enabled) {
          setProxyHint(
            view.active
              ? 'MTProto proxy is on.'
              : 'Proxy is enabled but not active — open settings to apply.',
          )
        }
      }
      catch {
        // ignore — login still works without the hint
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <div className="hifi-bg relative flex min-h-screen justify-center overflow-y-auto px-5 pb-12 pt-16 sm:pt-20">
      <div className="relative w-full max-w-104 animate-fade-up">
        <div className="mb-8 text-center">
          <span
            aria-hidden
            className="equalizer mx-auto mb-5 flex h-7 w-8 items-end justify-center overflow-hidden contain-[layout] [&>span]:w-1"
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

        <div className="flex flex-col">{children}</div>

        <div className="mx-auto mt-6 w-75">
          <Collapsible open={proxyOpen} onOpenChange={setProxyOpen}>
            <CollapsibleTrigger
              className="flex h-auto w-full items-center justify-between rounded-md px-2 py-2 text-xs text-muted-foreground hover:bg-muted/40 hover:text-foreground"
            >
              <span>Connection / proxy</span>
              <ChevronDown
                className={`size-3.5 transition-transform ${proxyOpen ? 'rotate-180' : ''}`}
              />
            </CollapsibleTrigger>
            <CollapsibleContent className="mt-2 rounded-lg border border-border/70 bg-muted/20 px-3 py-3">
              {proxyHint
                ? (
                    <p className="mb-3 text-xs text-muted-foreground">{proxyHint}</p>
                  )
                : (
                    <p className="mb-3 text-xs text-muted-foreground">
                      Paste the
                      {' '}
                      <span className="font-mono">tg://proxy</span>
                      {' '}
                      link from tg-ws-proxy tray (Copy link). Check
                      {' '}
                      <span className="font-medium text-foreground">Use MTProto proxy</span>
                      , then
                      {' '}
                      <span className="font-medium text-foreground">Apply & reconnect</span>
                      .
                    </p>
                  )}
              <ProxySettingsForm
                compact
                onApplied={(view) => {
                  if (view.applyError) {
                    setProxyHint(
                      'Proxy still unreachable. Disable it or check the helper is running.',
                    )
                  }
                  else if (view.active) {
                    setProxyHint('Proxy connected. Try signing in again.')
                  }
                  else {
                    setProxyHint('Proxy disabled. Connecting directly.')
                  }
                }}
              />
            </CollapsibleContent>
          </Collapsible>
        </div>

        <p className="mx-auto mt-6 max-w-xs text-center text-xs leading-relaxed text-muted-foreground">
          SoundGrammy is an independent app that uses the Telegram API. It is not affiliated with or endorsed by Telegram.
        </p>
      </div>
    </div>
  )
}
