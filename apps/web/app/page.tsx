import { getSession } from "../lib/auth";
import { getTracksByUser } from "../lib/db";
import { MusicLibrary } from "../components/MusicLibrary";
import styles from "./page.module.css";

export default async function HomePage() {
  const session = await getSession();

  if (!session) {
    return null; // middleware redirects to /login
  }

  const tracks = getTracksByUser(session.tgUserId);

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.logo}>SoundGrammy</h1>
        <span className={styles.user}>{session.firstName}</span>
      </header>
      <main className={styles.main}>
        <MusicLibrary tracks={tracks} />
      </main>
    </div>
  );
}
