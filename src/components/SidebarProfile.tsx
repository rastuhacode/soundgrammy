import { LogOut, RadioTower, Settings } from 'lucide-react'
import { useState } from 'react'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { SyncStatusDot } from '@/components/SyncStatusDot'
import { SettingsDialog } from '@/components/settings/SettingsDialog'
import { useProfileMusicSync } from '@/hooks/use-profile-music-sync'
import { useUserAvatar } from '@/hooks/use-user-avatar'
import { formatDisplayName, formatInitials, useSessionStore } from '@/stores/session-store'
import { api } from '@/lib/api'
import { Button } from '@/components/ui/button'

interface SidebarProfileProps {
  onLogout: () => void
}

export function SidebarProfile({ onLogout }: SidebarProfileProps) {
  const [isLoggingOut, setIsLoggingOut] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const session = useSessionStore(state => state.session)
  const avatarSrc = useUserAvatar(session?.tgUserId ?? null)
  const {
    phase,
    statusLabel,
    statusDetail,
    lastSyncDetail,
    requestSync,
    isSyncing,
  } = useProfileMusicSync()

  if (!session) return null

  const displayName = formatDisplayName(session)
  const initials = formatInitials(session)

  const handleLogout = async () => {
    setIsLoggingOut(true)
    try {
      await api.logout()
      onLogout()
    }
    finally {
      setIsLoggingOut(false)
    }
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger render={(
          <button
            type="button"
            className="relative shrink-0 rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
            onClick={e => e.stopPropagation()}
          >
            <Avatar size="lg" className="ring ring-primary/25">
              {avatarSrc ? <AvatarImage src={avatarSrc} alt={displayName} /> : null}
              <AvatarFallback className="bg-primary/15 text-sm font-medium text-primary">
                {initials}
              </AvatarFallback>
            </Avatar>
            <SyncStatusDot phase={phase} />
          </button>
        )}
        />

        <DropdownMenuContent
          sideOffset={5}
          align="center"
          className="w-64 overflow-hidden rounded-xl border-border/80 bg-popover/95 p-0 backdrop-blur-md"
        >
          <div className="border-b border-border/60 bg-primary/6 px-3 py-3">
            <div className="flex items-center gap-3">
              <div className="relative shrink-0">
                <Avatar className="size-11 ring-2 ring-primary/20">
                  {avatarSrc
                    ? (
                        <AvatarImage src={avatarSrc} alt={displayName} />
                      )
                    : null}
                  <AvatarFallback className="bg-primary/15 text-base font-medium text-primary">
                    {initials}
                  </AvatarFallback>
                </Avatar>
                <SyncStatusDot phase={phase} />
              </div>
              <div className="min-w-0">
                <p className="truncate text-sm font-semibold text-foreground">
                  {displayName}
                </p>
                {session.username
                  ? (
                      <p className="truncate font-mono text-[10px] uppercase tracking-[0.14em] text-primary/80">
                        @
                        {session.username}
                      </p>
                    )
                  : null}
              </div>
            </div>
          </div>

          <div className="p-1.5">
            <DropdownMenuGroup>
              <div className="flex items-center gap-2.5 rounded-lg py-2">
                <Button
                  variant="ghost"
                  size="icon-sm"
                  disabled={isSyncing}
                  aria-label={isSyncing ? 'Syncing Telegram music' : 'Sync Telegram music now'}
                  title={isSyncing ? 'Syncing…' : 'Sync now'}
                  onClick={requestSync}
                >
                  <RadioTower className={isSyncing ? 'animate-pulse text-primary' : 'text-primary/80'} />
                </Button>
                <div className="flex min-w-0 flex-col gap-0.5">
                  <span className="text-xs text-muted-foreground">Sync status</span>
                  <span
                    className={phase === 'error'
                      ? 'font-mono text-[10px] uppercase tracking-[0.14em] text-destructive'
                      : 'font-mono text-[10px] uppercase tracking-[0.14em] text-primary'}
                  >
                    {statusLabel}
                  </span>
                  <span
                    className={phase === 'error'
                      ? 'text-xs leading-snug text-destructive'
                      : 'text-xs leading-snug text-foreground'}
                    title={statusDetail}
                  >
                    {statusDetail}
                  </span>
                  {phase === 'offline' || phase === 'error'
                    ? (
                        <span className="text-xs leading-snug text-muted-foreground">
                          {lastSyncDetail}
                        </span>
                      )
                    : null}
                </div>
              </div>

            </DropdownMenuGroup>

            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => setSettingsOpen(true)}>
              <Settings className="size-4" />
              Settings
            </DropdownMenuItem>
            <DropdownMenuItem
              variant="destructive"
              disabled={isLoggingOut}
              onClick={handleLogout}
            >
              <LogOut className="size-4" />
              {isLoggingOut ? 'Signing out…' : 'Log out'}
            </DropdownMenuItem>

          </div>
        </DropdownMenuContent>
      </DropdownMenu>

      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </>
  )
}
