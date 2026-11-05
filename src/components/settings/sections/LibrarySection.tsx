import { useState } from 'react'
import { SettingsSection } from '../SettingsSection'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { useConnectivityStore } from '@/stores/connectivity-store'

export function LibrarySection() {
  const [confirm, setConfirm] = useState(false)
  const syncing = useConnectivityStore(state => state.syncing)
  const requestSync = useConnectivityStore(state => state.requestSync)
  return (
    <SettingsSection title="Music library">
      <p className="mb-3 text-xs text-muted-foreground">
        Refetch every track from Telegram. Your tracks will be hidden until indexing finishes. Playlists and likes are kept.
      </p>
      <Button variant="outline" disabled={syncing} onClick={() => setConfirm(true)}>Fully reindex tracks</Button>
      <Dialog open={confirm} onOpenChange={setConfirm}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Fully reindex your library?</DialogTitle>
            <DialogDescription>
              All track information will be fetched again. A progress loader will replace your library until every track is indexed.
              Your playlists, likes and cached audio will be kept for tracks still saved on Telegram.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirm(false)}>Cancel</Button>
            <Button
              disabled={syncing}
              onClick={() => {
                setConfirm(false)
                void requestSync('full').catch(() => {})
              }}
            >
              Confirm full reindex
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </SettingsSection>
  )
}
