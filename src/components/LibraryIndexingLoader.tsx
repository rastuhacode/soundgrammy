import { Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { SidebarDrawer } from '@/components/SidebarDrawer'
import { useLibraryStore } from '@/stores/library-store'
import { useConnectivityStore } from '@/stores/connectivity-store'

/** Initial and explicit full indexing replace every track view until completion. */
export function LibraryIndexingLoader({ onLogout }: { onLogout: () => void }) {
  const status = useLibraryStore(state => state.status)
  const libraryError = useLibraryStore(state => state.error)
  const { progress, syncing, syncError, syncMode, requestSync } = useConnectivityStore()
  const done = progress?.done ?? status?.processed ?? 0
  const total = progress?.total || status?.total || 0
  const error = syncError || libraryError
  return (
    <div className="hifi-bg flex h-dvh w-full flex-col">
      <header className="p-4"><SidebarDrawer onLogout={onLogout} /></header>
      <main className="flex grow items-center justify-center px-6">
        <div className="flex w-full max-w-sm flex-col items-center gap-4 text-center">
          <Loader2 className="size-8 animate-spin text-primary" aria-hidden="true" />
          <h1 className="text-lg font-semibold">Indexing your library</h1>
          <p role="status" className="text-sm text-muted-foreground">
            {total > 0 ? `${done} of ${total} tracks indexed` : 'Preparing your library…'}
          </p>
          <div
            role="progressbar"
            aria-label="Library indexing progress"
            aria-valuemin={0}
            aria-valuemax={total || undefined}
            aria-valuenow={total ? Math.min(done, total) : undefined}
            className="h-2 w-full overflow-hidden rounded-full bg-muted"
          >
            <div
              className={`h-full rounded-full bg-primary transition-[width] ${total ? '' : 'animate-pulse'}`}
              style={{ width: total ? `${Math.min(100, done / total * 100)}%` : '30%' }}
            />
          </div>
          <p className="text-sm text-muted-foreground">Your tracks will appear when indexing finishes.</p>
          {error && (
            <>
              <p role="alert" className="text-sm text-destructive">{error}</p>
              <Button
                disabled={syncing}
                onClick={() => {
                  const mode = status?.blocking ? 'soft' : syncMode
                  void requestSync(mode).catch(() => {})
                }}
              >
                Retry indexing
              </Button>
            </>
          )}
        </div>
      </main>
    </div>
  )
}
