import type { SyncPhase } from '@/hooks/use-profile-music-sync'

function OnlineDot() {
  return (
    <span className="absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full border-2 border-background bg-primary shadow-[0_0_8px_color-mix(in_oklch,var(--primary)_70%,transparent)]" />
  )
}

function SyncingDot() {
  return (
    <span className="absolute -bottom-0.5 -right-0.5 flex size-2.5">
      <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary opacity-75" />
      <span className="relative inline-flex size-2.5 rounded-full border-2 border-background bg-primary shadow-[0_0_8px_color-mix(in_oklch,var(--primary)_70%,transparent)]" />
    </span>
  )
}

function ConnectingDot() {
  return (
    <span className="absolute -bottom-0.5 -right-0.5 size-2.5 animate-pulse rounded-full border-2 border-background bg-muted-foreground/55" />
  )
}

function OfflineDot() {
  return (
    <span className="absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full border-2 border-background bg-muted-foreground/40" />
  )
}

function ErrorDot() {
  return (
    <span className="absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full border-2 border-background bg-destructive shadow-[0_0_8px_color-mix(in_oklch,var(--destructive)_60%,transparent)]" />
  )
}

export function SyncStatusDot({ phase }: { phase: SyncPhase }) {
  if (phase === 'connecting') {
    return <ConnectingDot />
  }
  if (phase === 'offline') {
    return <OfflineDot />
  }
  if (phase === 'syncing') {
    return <SyncingDot />
  }
  if (phase === 'error') {
    return <ErrorDot />
  }
  return <OnlineDot />
}
