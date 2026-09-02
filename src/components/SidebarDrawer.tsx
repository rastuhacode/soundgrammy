import {
  LogOut,
  Menu,
  RadioTower,
  Settings,
} from 'lucide-react'
import { useEffect, useState } from 'react'
import { getVersion } from '@tauri-apps/api/app'
import { api } from '@/lib/api'
import { useProfileMusicSync } from '@/hooks/use-profile-music-sync'
import { useUserAvatar } from '@/hooks/use-user-avatar'
import {
  formatDisplayName,
  formatInitials,
  useSessionStore,
} from '@/stores/session-store'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogMedia,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
  DrawerTrigger,
} from '@/components/ui/drawer'
import { SettingsDialog } from '@/components/settings/SettingsDialog'
import { cn } from '@/lib/utils'
import { Separator } from './ui/separator'
import { TauriLink } from './tauri/TauriLink'
import { SyncStatusDot } from './SyncStatusDot'

interface SidebarDrawerProps {
  onLogout: () => void
}

interface SidebarDrawerItemProps {
  children: React.ReactNode
  onClick: () => void
  className?: string
  disabled?: boolean
  title?: string
}

function SidebarDrawerItem({ children, onClick, className, disabled, title }: SidebarDrawerItemProps) {
  return (
    <button
      type="button"
      aria-current="page"
      disabled={disabled}
      title={title}
      className={cn(
        'mx-2 flex min-h-12 items-center gap-4 rounded-lg px-2 text-left text-sm font-medium text-sidebar-accent-foreground outline-none transition-colors hover:bg-sidebar-accent/80 focus-visible:ring-2 focus-visible:ring-sidebar-ring/50',
        className,
      )}
      onClick={onClick}
    >
      {children}
    </button>
  )
}

export function SidebarDrawer({ onLogout }: SidebarDrawerProps) {
  const [drawerOpen, setDrawerOpen] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [logoutDialogOpen, setLogoutDialogOpen] = useState(false)
  const [isLoggingOut, setIsLoggingOut] = useState(false)
  const [version, setVersion] = useState('')

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

  useEffect(() => {
    getVersion().then(setVersion)
  }, [])

  const networkTitle = statusDetail + ((phase === 'offline' || phase === 'error') ? `: ${lastSyncDetail}` : '')

  if (!session) return null

  const displayName = formatDisplayName(session)
  const initials = formatInitials(session)

  const openSettings = () => {
    setDrawerOpen(false)
    setSettingsOpen(true)
  }

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

  const confirmLogout = () => {
    setDrawerOpen(false)
    setLogoutDialogOpen(true)
  }

  return (
    <>
      <Drawer
        open={drawerOpen}
        onOpenChange={setDrawerOpen}
        swipeDirection="left"
      >
        <DrawerTrigger
          render={(
            <Button
              variant="ghost"
              size="icon"
              aria-label="Open navigation menu"
              title="Menu"
              className="-ml-1 text-muted-foreground hover:text-foreground"
            >
              <Menu className="size-5" />
            </Button>
          )}
        />

        <DrawerContent className="rounded-none border-y-0 border-l-0 bg-sidebar text-sidebar-foreground w-80 [--drawer-inset:0px]">
          <DrawerHeader className="md:gap-4 gap-4 border-b border-sidebar-border p-4 text-left flex-row items-center">
            <Avatar className="size-16">
              {avatarSrc
                ? <AvatarImage src={avatarSrc} alt={displayName} />
                : null}
              <AvatarFallback className="bg-primary/15 text-xl font-semibold text-primary">
                {initials}
              </AvatarFallback>
            </Avatar>
            <div className="flex flex-col">
              <DrawerTitle className="truncate text-lg font-semibold">
                {displayName}
              </DrawerTitle>
              <DrawerDescription className="mt-0.5 truncate text-sm text-primary">
                {session.username ? `@${session.username}` : 'Telegram account'}
              </DrawerDescription>
            </div>
          </DrawerHeader>

          <nav
            aria-label="Main navigation"
            className="flex min-h-0 grow flex-col overflow-y-auto py-2"
          >

            <SidebarDrawerItem
              disabled={isSyncing}
              onClick={requestSync}
              title={networkTitle}
            >
              <span className="relative">
                <RadioTower
                  className={cn(
                    'mt-0.5 size-5 text-muted-foreground',
                    isSyncing && 'animate-pulse text-primary',
                    phase === 'live' && 'text-primary',
                  )}
                />
                <SyncStatusDot phase={phase} />
              </span>
              <span className="flex min-w-0 flex-col gap-0.5">
                <span className="text-sm font-medium">
                  Sync Telegram music
                </span>
                <span
                  className={cn(
                    'truncate text-xs text-muted-foreground',
                    phase === 'error' && 'text-destructive',
                  )}
                >
                  {statusLabel}
                </span>
                {phase === 'error' && (
                  <span className="text-xs text-muted-foreground">
                    {lastSyncDetail}
                  </span>
                )}
              </span>
            </SidebarDrawerItem>

            <SidebarDrawerItem onClick={openSettings}>
              <Settings className="size-5 text-muted-foreground" />
              <span>Settings</span>
            </SidebarDrawerItem>
          </nav>

          <Separator />

          <DrawerFooter className="gap-3 border-t border-sidebar-border p-0 pb-4 pt-2">
            <SidebarDrawerItem onClick={confirmLogout} disabled={isLoggingOut} className="text-destructive transition-colors hover:bg-destructive/10 focus-visible:ring-destructive/30 disabled:opacity-50">
              <LogOut className="size-5" />
              <span>Log out</span>
            </SidebarDrawerItem>
            <div className="px-5 text-xs leading-relaxed text-muted-foreground">
              <p className="font-medium text-sidebar-foreground/70">SoundGrammy</p>
              <TauriLink
                href="https://github.com/rastuhacode/soundgrammy/releases"
                className="hover:underline"
              >
                Version:
                {' '}
                {version}
              </TauriLink>
            </div>
          </DrawerFooter>
        </DrawerContent>
      </Drawer>

      <AlertDialog open={logoutDialogOpen} onOpenChange={setLogoutDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogMedia className="bg-destructive/10 text-destructive">
              <LogOut />
            </AlertDialogMedia>
            <AlertDialogTitle>Log out of SoundGrammy?</AlertDialogTitle>
            <AlertDialogDescription>
              Just checking if you really want to log out
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isLoggingOut}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={isLoggingOut}
              onClick={handleLogout}
            >
              {isLoggingOut ? 'Logging out…' : 'Log out'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </>
  )
}
