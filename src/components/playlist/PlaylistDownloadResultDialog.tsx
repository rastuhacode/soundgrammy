import { useState } from 'react'
import { ChevronDown } from 'lucide-react'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type {
  PlaylistDownloadFailed,
  PlaylistDownloadResult,
  PlaylistDownloadSucceeded,
} from '@/types'

function trackLabel(
  item: Pick<PlaylistDownloadSucceeded | PlaylistDownloadFailed, 'title' | 'performer'>,
  trackId: number,
): string {
  const title = item.title?.trim() || 'Unknown title'
  const performer = item.performer?.trim()
  return performer ? `${performer} — ${title}` : title || `Track ${trackId}`
}

function ResultList({
  label,
  count,
  children,
}: {
  label: string
  count: number
  children: React.ReactNode
}) {
  const [open, setOpen] = useState(false)
  if (count === 0) return null
  return (
    <Collapsible open={open} onOpenChange={setOpen} className="rounded-md border border-border">
      <CollapsibleTrigger className="flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm font-medium hover:bg-muted/50">
        <span>
          {label}
          {' '}
          (
          {count}
          )
        </span>
        <ChevronDown
          className={cn(
            'size-4 shrink-0 opacity-60 transition-transform',
            open && 'rotate-180',
          )}
        />
      </CollapsibleTrigger>
      <CollapsibleContent className="border-t border-border px-3 py-2">
        <ul className="max-h-40 space-y-1 overflow-y-auto text-sm text-muted-foreground">
          {children}
        </ul>
      </CollapsibleContent>
    </Collapsible>
  )
}

export interface PlaylistDownloadResultDialogProps {
  result: PlaylistDownloadResult | null
  playlistName?: string | null
  open: boolean
  onOpenChange: (open: boolean) => void
  onOpenFolder?: () => void
}

export function PlaylistDownloadResultDialog({
  result,
  playlistName,
  open,
  onOpenChange,
  onOpenFolder,
}: PlaylistDownloadResultDialogProps) {
  const succeeded = result?.succeeded.length ?? 0
  const failed = result?.failed.length ?? 0
  const canOpenFolder = Boolean(result?.folderPath && onOpenFolder)

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {playlistName
              ? `Download finished — ${playlistName}`
              : 'Download finished'}
          </DialogTitle>
          <DialogDescription>
            {succeeded}
            {' '}
            downloaded,
            {' '}
            {failed}
            {' '}
            failed.
          </DialogDescription>
        </DialogHeader>

        {result
          ? (
              <div className="flex flex-col gap-2">
                <ResultList label="Downloaded" count={succeeded}>
                  {result.succeeded.map(item => (
                    <li key={`ok-${item.trackId}-${item.fileName}`}>
                      {trackLabel(item, item.trackId)}
                    </li>
                  ))}
                </ResultList>
                <ResultList label="Not downloaded" count={failed}>
                  {result.failed.map(item => (
                    <li key={`fail-${item.trackId}-${item.error}`}>
                      <span>{trackLabel(item, item.trackId)}</span>
                      <span className="mt-0.5 block text-xs opacity-80">
                        {item.error}
                      </span>
                    </li>
                  ))}
                </ResultList>
              </div>
            )
          : null}

        <DialogFooter className="gap-2 sm:gap-0">
          {canOpenFolder
            ? (
                <Button variant="secondary" onClick={onOpenFolder}>
                  Open folder
                </Button>
              )
            : null}
          <Button onClick={() => onOpenChange(false)}>OK</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
