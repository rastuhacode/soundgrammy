import { useEffect } from 'react'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { usePlaylistJobsStore } from '@/stores/playlist-jobs-store'
import { PlaylistDownloadResultDialog } from './PlaylistDownloadResultDialog'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'

export function PlaylistJobResults() {
  const resultItem = usePlaylistJobsStore(state => state.resultQueue[0] ?? null)
  const dismissResult = usePlaylistJobsStore(state => state.dismissResult)
  const errorMessage = usePlaylistJobsStore(state => state.errorQueue[0] ?? null)
  const dismissError = usePlaylistJobsStore(state => state.dismissError)

  const handleOpenFolder = async () => {
    if (!resultItem?.result.folderPath) return
    try {
      await revealItemInDir(resultItem.result.folderPath)
    }
    catch {
      // Ignore; user can browse Downloads manually.
    }
  }

  useEffect(() => {
    if (resultItem?.result.folderPath) {
      void revealItemInDir(resultItem.result.folderPath).catch(() => {})
    }
  }, [resultItem])

  return (
    <>

      <PlaylistDownloadResultDialog
        result={resultItem?.result ?? null}
        playlistName={resultItem?.playlistName ?? null}
        open={resultItem !== null}
        onOpenChange={(open) => {
          if (!open) dismissResult()
        }}
        onOpenFolder={handleOpenFolder}
      />

      <Dialog
        open={errorMessage !== null}
        onOpenChange={(open) => {
          if (!open) dismissError()
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Couldn’t finish</DialogTitle>
            <DialogDescription className="whitespace-pre-wrap">
              {errorMessage}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="secondary" onClick={() => dismissError()}>
              OK
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
