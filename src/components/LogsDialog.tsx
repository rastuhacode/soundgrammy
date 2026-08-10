import { useState } from 'react'
import { Check, Copy, Trash2 } from 'lucide-react'
import { appLogger, formatLogEntries, formatLogEntry } from '@/lib/app-logger'
import { useLogStore } from '@/stores/log-store'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

export interface LogsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

function fallbackCopy(text: string): boolean {
  const textarea = document.createElement('textarea')
  textarea.value = text
  textarea.style.position = 'fixed'
  textarea.style.opacity = '0'
  document.body.append(textarea)
  textarea.select()
  try {
    return document.execCommand('copy')
  }
  finally {
    textarea.remove()
  }
}

async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text)
  }
  catch (error) {
    if (fallbackCopy(text)) return
    throw error
  }
}

export function LogsDialog({ open, onOpenChange }: LogsDialogProps) {
  const entries = useLogStore(state => state.entries)
  const clear = useLogStore(state => state.clear)
  const [copied, setCopied] = useState<string | null>(null)

  const handleCopy = async (key: string, text: string) => {
    try {
      await copyText(text)
      setCopied(key)
      window.setTimeout(() => setCopied(current => current === key ? null : current), 1_500)
    }
    catch (error) {
      appLogger.error({
        source: 'ui',
        title: 'Could not copy diagnostic logs',
        error,
      })
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[85vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-3xl">
        <DialogHeader className="shrink-0 border-b border-border/60 px-4 py-4 pr-12">
          <DialogTitle>Diagnostic logs</DialogTitle>
          <DialogDescription>
            Logs stay on this device until they are cleared. Review them before sharing.
          </DialogDescription>
        </DialogHeader>

        <div className="flex shrink-0 items-center justify-between gap-3 border-b border-border/60 px-4 py-3">
          <span className="font-mono text-xs text-muted-foreground">
            {entries.length}
            {' '}
            entr
            {entries.length === 1 ? 'y' : 'ies'}
          </span>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={entries.length === 0}
              onClick={() => handleCopy('all', formatLogEntries(entries))}
            >
              {copied === 'all' ? <Check className="size-4" /> : <Copy className="size-4" />}
              {copied === 'all' ? 'Copied' : 'Copy all'}
            </Button>
            <Button
              variant="destructive"
              size="sm"
              disabled={entries.length === 0}
              onClick={clear}
            >
              <Trash2 className="size-4" />
              Clear
            </Button>
          </div>
        </div>

        <div className="min-h-0 grow overflow-y-auto bg-muted/20 p-4">
          {entries.length === 0
            ? (
                <div className="flex min-h-48 items-center justify-center text-sm text-muted-foreground">
                  No diagnostic logs recorded.
                </div>
              )
            : (
                <div className="flex flex-col gap-3">
                  {[...entries].reverse().map(entry => (
                    <article
                      key={entry.id}
                      className="relative overflow-hidden rounded-lg border border-border/70 bg-background"
                    >
                      <Button
                        variant="ghost"
                        size="icon-xs"
                        className="absolute right-2 top-2"
                        aria-label={`Copy ${entry.title}`}
                        onClick={() => handleCopy(entry.id, formatLogEntry(entry))}
                      >
                        {copied === entry.id ? <Check /> : <Copy />}
                      </Button>
                      <pre className="max-h-80 overflow-auto whitespace-pre-wrap wrap-break-word p-3 pr-10 font-mono text-[11px] leading-relaxed text-foreground">
                        {formatLogEntry(entry)}
                      </pre>
                    </article>
                  ))}
                </div>
              )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
