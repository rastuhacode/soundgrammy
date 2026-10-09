import { useEffect, useId, useState } from 'react'
import type { QueueSaveScope } from '@/lib/queue'
import { trackIdsForSaveScope } from '@/lib/queue'
import { errorMessage } from '@/lib/errors'
import { validatePlaylistName } from '@/lib/playlist-form'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { useAsyncScope } from '@/hooks/use-async-scope'
import { usePlayerStore } from '@/stores/player-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { cn } from '@/lib/utils'

const SCOPES: { id: QueueSaveScope, label: string, description: string }[] = [
  {
    id: 'full',
    label: 'Full queue',
    description: 'History, now playing, and up next',
  },
  {
    id: 'fromHere',
    label: 'From here',
    description: 'Now playing and everything after',
  },
  {
    id: 'upNext',
    label: 'Up next only',
    description: 'Only tracks after the current one',
  },
]

export interface SaveQueueAsPlaylistDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function SaveQueueAsPlaylistDialog({
  open,
  onOpenChange,
}: SaveQueueAsPlaylistDialogProps) {
  const formId = useId()
  const { capture, invalidate } = useAsyncScope(open)
  const handleOpenChange = (next: boolean) => {
    if (!next) invalidate()
    onOpenChange(next)
  }
  const queue = usePlayerStore(state => state.queue)
  const data = usePlaylistsStore(state => state.data)
  const createPlaylist = usePlaylistsStore(state => state.createPlaylist)
  const setSelectedPlaylist = usePlaylistsStore(
    state => state.setSelectedPlaylist,
  )

  const [name, setName] = useState('')
  const [scope, setScope] = useState<QueueSaveScope>('full')
  const [nameError, setNameError] = useState<string | null>(null)
  const [scopeError, setScopeError] = useState<string | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)

  useEffect(() => {
    if (!open) return
    /* eslint-disable react-hooks/set-state-in-effect -- Reset draft when dialog opens. */
    setIsSubmitting(false)
    setName('')
    setScope('full')
    setNameError(null)
    setScopeError(null)
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [open])

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!data || !open || isSubmitting) return
    const current = capture()

    const trimmed = name.trim()
    const issue = validatePlaylistName(name)
    if (issue) {
      setNameError(issue)
      return
    }
    setNameError(null)

    const trackIds = trackIdsForSaveScope(queue, scope)
    if (trackIds.length === 0) {
      setScopeError('Nothing to save for this scope')
      return
    }
    setScopeError(null)

    setIsSubmitting(true)
    try {
      const created = await createPlaylist(trimmed, trackIds)
      if (!current()) return
      setSelectedPlaylist(created.id)
      handleOpenChange(false)
    }
    catch (err) {
      if (!current()) return
      setScopeError(
        errorMessage(err),
      )
    }
    finally {
      if (current()) setIsSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="z-100 sm:max-w-md" overlayClassName="z-[90]">
        <DialogHeader>
          <DialogTitle>Save queue as playlist</DialogTitle>
          <DialogDescription>
            Create a new custom playlist from the current queue. The queue itself is unchanged.
          </DialogDescription>
        </DialogHeader>

        <form id={formId} className="flex flex-col gap-4" onSubmit={handleSubmit}>
          <div className="flex flex-col gap-2">
            <label htmlFor={`${formId}-name`} className="text-sm font-medium">
              Name
            </label>
            <Input
              id={`${formId}-name`}
              value={name}
              onChange={event => setName(event.target.value)}
              placeholder="Playlist name"
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="off"
              spellCheck={false}
              maxLength={100}
            />
            {nameError && (
              <p className="text-xs text-destructive">{nameError}</p>
            )}
          </div>

          <div className="flex flex-col gap-2">
            <span className="text-sm font-medium" id={`${formId}-scope-label`}>
              Include
            </span>
            <RadioGroup
              aria-labelledby={`${formId}-scope-label`}
              value={scope}
              onValueChange={(value) => {
                if (
                  value === 'full'
                  || value === 'fromHere'
                  || value === 'upNext'
                ) {
                  setScope(value)
                }
              }}
              className="gap-2"
            >
              {SCOPES.map(option => (
                <label
                  key={option.id}
                  className={cn(
                    'flex cursor-pointer items-start gap-3 rounded-md border border-border px-3 py-2',
                    scope === option.id && 'border-primary bg-primary/5',
                  )}
                >
                  <RadioGroupItem
                    value={option.id}
                    className="mt-0.5"
                    aria-label={option.label}
                  />
                  <span className="min-w-0">
                    <span className="block text-sm font-medium">{option.label}</span>
                    <span className="block text-xs text-muted-foreground">
                      {option.description}
                    </span>
                  </span>
                </label>
              ))}
            </RadioGroup>
            {scopeError && (
              <p className="text-xs text-destructive">{scopeError}</p>
            )}
          </div>
        </form>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => handleOpenChange(false)}
            disabled={isSubmitting}
          >
            Cancel
          </Button>
          <Button type="submit" form={formId} disabled={isSubmitting}>
            {isSubmitting ? 'Saving…' : 'Save playlist'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
