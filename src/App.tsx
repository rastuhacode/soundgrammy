import { useCallback, useEffect, useRef, useState } from "react";
import { Loader2 } from "lucide-react";
import { MtprotoLogin } from "@/components/auth/MtprotoLogin";
import { PlayerSidebar } from "@/components/PlayerSidebar";
import { PlaylistView } from "@/components/playlist/PlaylistView";
import { AudioPlayer } from "@/components/audio/AudioPlayer";
import { api, onSyncDone } from "@/lib/api";
import { authUserToSession, type AuthUser } from "@/types";
import { useLibraryStore } from "@/stores/library-store";
import { usePlayerStore } from "@/stores/player-store";
import { usePlaylistsStore } from "@/stores/playlists-store";
import { useSessionStore } from "@/stores/session-store";

type AppStatus = "loading" | "login" | "ready";

/** Refreshes the library + playlists from the backend into the stores. */
async function loadLibrary(firstLoad: boolean) {
  const [tracks, playlists] = await Promise.all([
    api.listTracks(),
    api.listPlaylists(),
  ]);

  useLibraryStore.getState().setTracks(tracks);

  // Keep the current track reference fresh (mirrors PlayerTracksHydrator).
  const { currentTrack } = usePlayerStore.getState();
  if (currentTrack) {
    const refreshed = tracks.find((t) => t.id === currentTrack.id) ?? null;
    if (refreshed !== currentTrack) {
      usePlayerStore.setState({
        currentTrack: refreshed,
        ...(refreshed ? {} : { isPlaying: false }),
      });
    }
  }

  if (firstLoad) {
    usePlaylistsStore.getState().hydrate(playlists);
  } else {
    usePlaylistsStore.getState().setData(playlists);
  }
  usePlaylistsStore.getState().syncQueueToPlayer();
}

export default function App() {
  const [status, setStatus] = useState<AppStatus>("loading");
  const session = useSessionStore((state) => state.session);
  const setSession = useSessionStore((state) => state.setSession);
  const clearSession = useSessionStore((state) => state.clearSession);
  const syncStartedRef = useRef(false);

  const runSync = useCallback(async () => {
    if (syncStartedRef.current) return;
    syncStartedRef.current = true;
    try {
      const result = await api.syncSavedMusic();
      if (result.changed) {
        await loadLibrary(false);
      }
    } catch {
      // sync failures leave the cached library intact
    }
  }, []);

  const bootstrap = useCallback(async () => {
    try {
      const authStatus = await api.authStatus();
      if (!authStatus.authorized || !authStatus.user) {
        setStatus("login");
        return;
      }
      setSession(authUserToSession(authStatus.user));
      await loadLibrary(true);
      setStatus("ready");
      void runSync();
    } catch {
      setStatus("login");
    }
  }, [setSession, runSync]);

  useEffect(() => {
    // Bootstrap synchronizes React state with persisted backend auth/library state.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    bootstrap();
  }, [bootstrap]);

  // Reload the library whenever the backend reports a completed sync.
  useEffect(() => {
    if (status !== "ready") return;
    const promise = onSyncDone(() => {
      void loadLibrary(false);
    });
    return () => {
      void promise.then((unlisten) => unlisten());
    };
  }, [status]);

  const handleAuthenticated = useCallback(
    async (user: AuthUser) => {
      setSession(authUserToSession(user));
      syncStartedRef.current = false;
      try {
        await loadLibrary(true);
      } catch {
        // ignore; sync will populate
      }
      setStatus("ready");
      void runSync();
    },
    [setSession, runSync],
  );

  const handleLogout = useCallback(() => {
    clearSession();
    useLibraryStore.getState().setTracks([]);
    usePlayerStore.setState({
      orderedTracks: [],
      tracks: [],
      currentTrack: null,
      isPlaying: false,
    });
    usePlaylistsStore.setState({ data: null, queueSnapshot: null });
    syncStartedRef.current = false;
    setStatus("login");
  }, [clearSession]);

  if (status === "loading") {
    return (
      <div className="hifi-bg flex min-h-screen items-center justify-center">
        <Loader2 className="size-6 animate-spin text-primary" />
      </div>
    );
  }

  if (status === "login" || !session) {
    return <MtprotoLogin onAuthenticated={handleAuthenticated} />;
  }

  return (
    <div className="hifi-bg flex h-screen w-screen flex-col overflow-hidden">
      <div className="flex min-h-0 flex-1">
        <aside className="w-80 shrink-0 border-r border-border bg-sidebar/60 backdrop-blur-sm">
          <PlayerSidebar onLogout={handleLogout} />
        </aside>
        <main className="flex min-h-0 flex-1 flex-col">
          <PlaylistView />
        </main>
      </div>
      <AudioPlayer />
    </div>
  );
}
