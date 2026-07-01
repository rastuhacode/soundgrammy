import { Hash, LogOut, RadioTower, UserRound } from "lucide-react";
import { useState } from "react";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { SyncStatusDot } from "@/components/SyncStatusDot";
import { useProfileMusicSync } from "@/hooks/use-profile-music-sync";
import { useUserAvatar } from "@/hooks/use-user-avatar";
import {
  formatDisplayName,
  formatInitials,
  useSessionStore,
} from "@/stores/session-store";
import { api } from "@/lib/api";

interface SidebarProfileProps {
  onLogout: () => void;
}

export function SidebarProfile({ onLogout }: SidebarProfileProps) {
  const [isLoggingOut, setIsLoggingOut] = useState(false);
  const session = useSessionStore((state) => state.session);
  const avatarSrc = useUserAvatar(Boolean(session));
  const { phase, statusLabel, statusDetail } = useProfileMusicSync();

  if (!session) return null;

  const displayName = formatDisplayName(session);
  const initials = formatInitials(session);

  const handleLogout = async () => {
    setIsLoggingOut(true);
    try {
      await api.logout();
      onLogout();
    } finally {
      setIsLoggingOut(false);
    }
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={(
        <button
          type="button"
          className="relative shrink-0 rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          onClick={(e) => e.stopPropagation()}
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
        align="end"
        className="w-64 overflow-hidden rounded-xl border-border/80 bg-popover/95 p-0 shadow-lg backdrop-blur-md"
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
            <DropdownMenuLabel>Account</DropdownMenuLabel>

            <div className="flex items-center gap-2.5 rounded-lg px-2.5 py-2">
              <UserRound className="size-3.5 text-primary/80" />
              <span className="flex min-w-0 flex-col gap-0.5">
                <span className="text-xs text-muted-foreground">Display name</span>
                <span className="truncate text-sm text-foreground">
                  {displayName}
                </span>
              </span>
            </div>

            {session.username
              ? (
                  <div className="flex items-center gap-2.5 rounded-lg px-2.5 py-2">
                    <span className="flex size-3.5 items-center justify-center font-mono text-[10px] text-primary/80">
                      @
                    </span>
                    <span className="flex min-w-0 flex-col gap-0.5">
                      <span className="text-xs text-muted-foreground">Username</span>
                      <span className="truncate font-mono text-sm text-foreground">
                        @
                        {session.username}
                      </span>
                    </span>
                  </div>
                )
              : null}

            <div className="flex items-center gap-2.5 rounded-lg px-2.5 py-2">
              <Hash className="size-3.5 text-primary/80" />
              <span className="flex min-w-0 flex-col gap-0.5">
                <span className="text-xs text-muted-foreground">Telegram ID</span>
                <span className="truncate font-mono text-sm text-foreground">
                  {session.tgUserId}
                </span>
              </span>
            </div>
          </DropdownMenuGroup>
          <DropdownMenuSeparator />
          <DropdownMenuGroup>
            <DropdownMenuLabel>Telegram</DropdownMenuLabel>

            <div className="flex items-center gap-2.5 rounded-lg px-2.5 py-2">
              <RadioTower className="size-3.5 text-primary/80" />
              <span className="flex min-w-0 flex-col gap-0.5">
                <span className="text-xs text-muted-foreground">Sync status</span>
                <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-primary">
                  {statusLabel}
                </span>
                <span className="text-sm leading-snug text-foreground">
                  {statusDetail}
                </span>
              </span>
            </div>
          </DropdownMenuGroup>

          <DropdownMenuSeparator />

          <DropdownMenuItem
            variant="destructive"
            disabled={isLoggingOut}
            onClick={(event) => {
              event.preventDefault();
              void handleLogout();
            }}
          >
            <LogOut className="size-4" />
            {isLoggingOut ? "Signing out…" : "Log out"}
          </DropdownMenuItem>

        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
