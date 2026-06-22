import { redirect } from "next/navigation";
import { getSession } from "@/lib/auth";
import { getMtprotoSession, getTracksByUser } from "@/lib/db";
import { ensureProfileMusicSynced } from "@/lib/mtproto/sync";
import { PlayerTracksHydrator } from "@/components/hydrators/PlayerTracksHydrator";
import { SessionHydrator } from "@/components/hydrators/SessionHydrator";

export type PlayerLayoutProps = Readonly<{
  children: React.ReactNode;
  sidebar: React.ReactNode;
  player: React.ReactNode;
}>;

export default async function PlayerLayout({
  children,
  sidebar,
  player,
}: PlayerLayoutProps) {
  const session = await getSession();

  if (!session) {
    redirect("/login");
  }

  const mtprotoSession = getMtprotoSession(session.tgUserId);
  if (!mtprotoSession) {
    redirect("/login");
  }

  await ensureProfileMusicSynced(session.tgUserId);
  const tracks = getTracksByUser(session.tgUserId);

  return (
    <SessionHydrator session={session}>
      <PlayerTracksHydrator tracks={tracks}>
        <div className="flex h-full w-full flex-col">
          <div className="flex w-full grow overflow-hidden">
            <aside className="h-full w-96 shrink-0 overflow-y-auto border-r border-border bg-background">
              {sidebar}
            </aside>
            <main className="max-h-full grow overflow-y-auto">{children}</main>
          </div>
          <div className="shrink-0">{player}</div>
        </div>
      </PlayerTracksHydrator>
    </SessionHydrator>
  );
}
