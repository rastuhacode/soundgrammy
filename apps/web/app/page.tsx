import { redirect } from "next/navigation";
import { getSession } from "../lib/auth";
import { getMtprotoSession, getTracksByUser } from "../lib/db";
import { ensureProfileMusicSynced } from "../lib/mtproto/sync";
import { MusicLibrary } from "../components/MusicLibrary";
import { ProfileMusicSync } from "../components/ProfileMusicSync";

export default async function HomePage() {
  const session = await getSession();

  if (!session) {
    return null;
  }

  const mtprotoSession = getMtprotoSession(session.tgUserId);
  if (!mtprotoSession) {
    redirect("/login");
  }

  await ensureProfileMusicSynced(session.tgUserId);
  const tracks = getTracksByUser(session.tgUserId);

  return (
    <div className="min-h-screen min-w-screen flex flex-col pb-20 space-y-4">
      <header className="flex items-center justify-between p-4 border-b border-border">
        <h1 className="text-2xl font-bold">SoundGrammy</h1>
        <span className="opacity-60">{session.firstName}</span>
      </header>
      <ProfileMusicSync trackCount={tracks.length} />
      <main className="w-full grow">
        <MusicLibrary tracks={tracks} />
      </main>
    </div>
  );
}
