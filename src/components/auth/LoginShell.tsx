import { useEffect, useState, type ReactNode } from 'react'
import { api } from '@/lib/api'
import { ProxySettingsForm } from '@/components/settings/forms/ProxySettingsForm'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { getProxyDisplayState } from '@/lib/proxy-status'
import type { ProxySettingsView } from '@/types'

const proxyIndicatorClassNames = {
  checking: 'bg-muted-foreground/50',
  on: 'bg-emerald-500',
  off: 'bg-muted-foreground/50',
  pending: 'bg-amber-500',
  issue: 'bg-destructive',
} as const

export function LoginShell({ children }: { children: ReactNode }) {
  const [proxyDialogOpen, setProxyDialogOpen] = useState(false)
  const [proxyView, setProxyView] = useState<ProxySettingsView | null>(null)

  useEffect(() => {
    let cancelled = false
    api.getProxySettings()
      .then((view) => {
        if (!cancelled) setProxyView(view)
      })
      .catch(() => {
        // Login still works if proxy status cannot be loaded.
      })
    return () => {
      cancelled = true
    }
  }, [])

  const proxyDisplay = getProxyDisplayState(proxyView)

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
        </div>

        <div className="flex flex-col">{children}</div>

        <div className="mx-auto mt-6 w-75">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="w-full text-muted-foreground hover:text-foreground"
            onClick={() => setProxyDialogOpen(true)}
          >
            <span
              aria-hidden
              className={`size-2 rounded-full ${proxyIndicatorClassNames[proxyDisplay.kind]}`}
            />
            {proxyDisplay.label}
          </Button>
        </div>

        <Dialog open={proxyDialogOpen} onOpenChange={setProxyDialogOpen}>
          <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-md">
            <DialogHeader>
              <DialogTitle>Telegram proxy</DialogTitle>
              <DialogDescription>
                {proxyDisplay.description}
              </DialogDescription>
            </DialogHeader>
            <div className="rounded-lg border border-border/70 bg-muted/20 px-3 py-3">
              <ProxySettingsForm
                compact
                onApplied={setProxyView}
              />
            </div>
          </DialogContent>
        </Dialog>

        <p className="mx-auto mt-6 max-w-xs text-center text-xs leading-relaxed text-muted-foreground">
          SoundGrammy is an independent app that uses the Telegram API. It is not affiliated with or endorsed by Telegram.
        </p>
      </div>
    </div>
  )
}
