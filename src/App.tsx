import { Button } from '@/components/ui/button'
import { useCallback, useEffect, useState } from 'react'
import { Loader2 } from 'lucide-react'
import { MtprotoLogin } from '@/components/auth/MtprotoLogin'
import { PlayerSidebar } from '@/components/PlayerSidebar'
import { PlaylistView } from '@/components/playlist/PlaylistView'
import { AudioPlayer } from '@/components/audio/AudioPlayer'
import { useAppSession } from '@/hooks/use-app-session'
import { hydrateLibrary } from '@/lib/library-hydration'
import { useLibraryStore } from '@/stores/library-store'
import { type PlaylistId, usePlaylistsStore } from '@/stores/playlists-store'
import { useAndroidBackAction } from '@/hooks/use-android-back'
import { useCompactDisplay } from '@/hooks/use-compact-display'
import {
  SplitterGroup,
  SplitterPanel,
  SplitterResizeHandle,
} from '@/components/ui/splitter'

export default function App() {
  const { status, session, handleAuthenticated, resetToLogin } = useAppSession()
  const libraryError = useLibraryStore(state => state.error)
  const playlistsError = usePlaylistsStore(state => state.error)
  const { isCompact } = useCompactDisplay()
  const [compactPlaylistOpen, setCompactPlaylistOpen] = useState(false)
  useAndroidBackAction(status === 'ready' && isCompact && compactPlaylistOpen, () => {
    setCompactPlaylistOpen(false)
  })
  useEffect(() => {
    if (status !== 'ready') return
    return usePlaylistsStore.subscribe((next, previous) => {
      if (next.selectedPlaylistId !== previous.selectedPlaylistId) {
        setCompactPlaylistOpen(true)
      }
    })
  }, [status])

  const handleLogout = useCallback(() => {
    resetToLogin()
    setCompactPlaylistOpen(false)
  }, [resetToLogin])

  const handleSelectPlaylist = useCallback((id: PlaylistId) => {
    usePlaylistsStore.getState().setSelectedPlaylist(id)
    setCompactPlaylistOpen(true)
  }, [])

  if (status === 'loading') {
    return (
      <div className="hifi-bg flex min-h-screen items-center justify-center">
        <Loader2 className="size-6 animate-spin text-primary" />
      </div>
    )
  }

  if (status === 'login' || !session) {
    return <MtprotoLogin onAuthenticated={handleAuthenticated} />
  }

  return (
    <div className="hifi-bg flex h-dvh w-full flex-col overflow-hidden">
      {!isCompact
        ? (
            <SplitterGroup
              id="player-layout"
              autoSaveId="player-layout"
              panelIds={['playlist-sidebar', 'track-list']}
              className="min-h-0 grow"
            >
              <SplitterPanel
                id="playlist-sidebar"
                defaultSize={320}
                minSize={220}
                maxSize={480}
                groupResizeBehavior="preserve-pixel-size"
              >
                <aside className="size-full bg-sidebar/60 backdrop-blur-sm">
                  <PlayerSidebar onLogout={handleLogout} onSelectPlaylist={handleSelectPlaylist} />
                </aside>
              </SplitterPanel>
              <SplitterResizeHandle aria-label="Resize playlist sidebar" />
              <SplitterPanel id="track-list" minSize={400}>
                <main className="flex size-full min-h-0 flex-col">
                  <PlaylistView />
                </main>
              </SplitterPanel>
            </SplitterGroup>
          )
        : (
            <div className="min-h-0 min-w-0 grow">
              <aside className={compactPlaylistOpen ? 'hidden' : 'size-full bg-sidebar/60 backdrop-blur-sm'}>
                <PlayerSidebar onLogout={handleLogout} onSelectPlaylist={handleSelectPlaylist} />
              </aside>
              {compactPlaylistOpen
                ? (
                    <main className="flex size-full min-h-0 flex-col">
                      <PlaylistView onBack={() => setCompactPlaylistOpen(false)} />
                    </main>
                  )
                : null}
            </div>
          )}
      {(libraryError || playlistsError) && (
        <div role="alert" className="flex items-center gap-3 px-4 py-2 text-sm text-destructive">
          <span>{libraryError || playlistsError}</span>
          <Button variant="ghost" size="sm" onClick={() => void hydrateLibrary()}>Retry loading library</Button>
        </div>
      )}
      <AudioPlayer />
    </div>
  )
}
