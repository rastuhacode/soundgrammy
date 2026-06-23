"use client";

import { useMutation } from "@tanstack/react-query";
import { Hash, LogOut, RadioTower, UserRound } from "lucide-react";
import { useRouter } from "next/navigation";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import {
  DropdownMenu,
  DropdownMenuContent,
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
import { useTRPC } from "@/trpc/client";

interface SidebarProfileProps {
  trackCount: number;
}

export function SidebarProfile({ trackCount }: SidebarProfileProps) {
  const router = useRouter();
  const trpc = useTRPC();
  const session = useSessionStore((state) => state.session);
  const clearSession = useSessionStore((state) => state.clearSession);
  const avatarSrc = useUserAvatar(Boolean(session));
  const { phase, statusLabel, statusDetail } = useProfileMusicSync(trackCount);
  const logoutMutation = useMutation(trpc.auth.logout.mutationOptions());

  if (!session) return null;

  const displayName = formatDisplayName(session);
  const initials = formatInitials(session);

  const handleLogout = async () => {
    await logoutMutation.mutateAsync();
    clearSession();
    router.push("/login");
    router.refresh();
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger className="group flex shrink-0 items-center outline-none focus-visible:ring-2 focus-visible:ring-ring/50 rounded-full">
        <div className="relative shrink-0">
          <Avatar size="lg" className="ring ring-primary/25">
            {avatarSrc
              ? (
                  <AvatarImage src={avatarSrc} alt={displayName} />
                )
              : null}
            <AvatarFallback className="bg-primary/15 text-sm font-medium text-primary">
              {initials}
            </AvatarFallback>
          </Avatar>
          <SyncStatusDot phase={phase} />
        </div>
      </DropdownMenuTrigger>

      <DropdownMenuContent className="overflow-hidden rounded-xl border-border/80 bg-popover/95 p-0 shadow-lg backdrop-blur-md">
        <div className="border-b border-border/60 bg-primary/6 px-3 py-3">
          <div className="flex items-center gap-3">
            <div className="relative shrink-0">
              <Avatar className="size-11 ring-2 ring-primary/20">
                {avatarSrc
                  ? (
                      <AvatarImage src={avatarSrc} alt={displayName} />
                    )
                  : null}
                <AvatarFallback className="bg-primary/15 font-serif text-base font-medium text-primary">
                  {initials}
                </AvatarFallback>
              </Avatar>
              <SyncStatusDot phase={phase} />
            </div>
            <div className="min-w-0">
              <p className="truncate font-serif text-sm font-semibold text-foreground">
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
          <DropdownMenuLabel className="px-2 py-1.5 font-mono text-[10px] font-normal uppercase tracking-[0.16em] text-muted-foreground">
            Account
          </DropdownMenuLabel>

          <DropdownMenuItem
            disabled
            className="cursor-default gap-2.5 rounded-lg px-2.5 py-2 focus:bg-transparent"
          >
            <UserRound className="size-3.5 text-primary/80" />
            <span className="flex min-w-0 flex-col gap-0.5">
              <span className="text-xs text-muted-foreground">
                Display name
              </span>
              <span className="truncate text-sm text-foreground">
                {displayName}
              </span>
            </span>
          </DropdownMenuItem>

          {session.username
            ? (
                <DropdownMenuItem
                  disabled
                  className="cursor-default gap-2.5 rounded-lg px-2.5 py-2 focus:bg-transparent"
                >
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
                </DropdownMenuItem>
              )
            : null}

          <DropdownMenuItem
            disabled
            className="cursor-default gap-2.5 rounded-lg px-2.5 py-2 focus:bg-transparent"
          >
            <Hash className="size-3.5 text-primary/80" />
            <span className="flex min-w-0 flex-col gap-0.5">
              <span className="text-xs text-muted-foreground">Telegram ID</span>
              <span className="truncate font-mono text-sm text-foreground">
                {session.tgUserId}
              </span>
            </span>
          </DropdownMenuItem>

          <DropdownMenuSeparator className="my-1.5 bg-border/70" />

          <DropdownMenuLabel className="px-2 py-1.5 font-mono text-[10px] font-normal uppercase tracking-[0.16em] text-muted-foreground">
            Telegram
          </DropdownMenuLabel>

          <DropdownMenuItem
            disabled
            className="cursor-default gap-2.5 rounded-lg px-2.5 py-2 focus:bg-transparent"
          >
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
          </DropdownMenuItem>

          <DropdownMenuSeparator className="my-1.5 bg-border/70" />

          <DropdownMenuItem
            variant="destructive"
            disabled={logoutMutation.isPending}
            onSelect={(event) => {
              event.preventDefault();
              void handleLogout();
            }}
            className="gap-2.5 rounded-lg px-2.5 py-2"
          >
            <LogOut className="size-4" />
            {logoutMutation.isPending ? "Signing out…" : "Log out"}
          </DropdownMenuItem>
        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
