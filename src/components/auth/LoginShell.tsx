import type { ReactNode } from 'react'

export function LoginShell({ children }: { children: ReactNode }) {
  return (
    <div className="hifi-bg relative flex min-h-screen justify-center overflow-y-auto px-5 pb-12 pt-16 sm:pt-20">
      <div className="pointer-events-none absolute left-1/2 top-[-10%] h-144 w-xl -translate-x-1/2 rounded-full bg-primary/15 blur-[120px] animate-glow-pulse" />

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

        <p className="mt-6 text-center font-mono text-[10px] uppercase tracking-[0.3em] text-muted-foreground/60">
          End-to-end via MTProto
        </p>
      </div>
    </div>
  )
}
