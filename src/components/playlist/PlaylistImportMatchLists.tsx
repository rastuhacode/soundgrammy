import { useState } from 'react'
import { ChevronDown } from 'lucide-react'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { cn } from '@/lib/utils'
import type {
  PlaylistImportFailed,
  PlaylistImportSucceeded,
} from '@/types'

function trackLabel(
  item: Pick<PlaylistImportSucceeded | PlaylistImportFailed, 'title' | 'performer' | 'fileUniqueId'>,
): string {
  const title = item.title?.trim() || 'Unknown title'
  const performer = item.performer?.trim()
  return performer ? `${performer} — ${title}` : title
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

export interface PlaylistImportMatchListsProps {
  succeeded: PlaylistImportSucceeded[]
  failed: PlaylistImportFailed[]
  addedLabel?: string
  missingLabel?: string
  /** When false, only expandable lists (no summary line). */
  showSummary?: boolean
}

export function PlaylistImportMatchLists({
  succeeded,
  failed,
  addedLabel = 'Will add',
  missingLabel = 'Not in library',
  showSummary = true,
}: PlaylistImportMatchListsProps) {
  return (
    <div className="flex flex-col gap-2">
      {showSummary
        ? (
            <p className="text-sm text-muted-foreground">
              {succeeded.length}
              {' '}
              songs will be added
              {failed.length > 0
                ? (
                    <>
                      ,
                      {' '}
                      {failed.length}
                      {' '}
                      songs will be missing
                    </>
                  )
                : null}
              .
            </p>
          )
        : null}
      <ResultList label={addedLabel} count={succeeded.length}>
        {succeeded.map(item => (
          <li key={`ok-${item.fileUniqueId}-${item.title}`}>
            {trackLabel(item)}
          </li>
        ))}
      </ResultList>
      <ResultList label={missingLabel} count={failed.length}>
        {failed.map(item => (
          <li key={`fail-${item.fileUniqueId}-${item.reason}`}>
            <span>{trackLabel(item)}</span>
            <span className="mt-0.5 block text-xs opacity-80">
              {item.reason === 'notInLibrary'
                ? 'Not in your Saved Music library'
                : item.reason}
            </span>
          </li>
        ))}
      </ResultList>
    </div>
  )
}
