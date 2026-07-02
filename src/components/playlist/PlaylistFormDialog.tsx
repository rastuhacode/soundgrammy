import { useEffect, useId, useRef, useState } from 'react'
import { ImagePlus, Trash2 } from 'lucide-react'
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
import type { CustomPlaylistSummary } from '@/lib/db'
import { usePlaylistThumbnail } from '@/hooks/use-playlist-thumbnail'
import {
  createThumbnailPreviewUrl,
  readPlaylistThumbnailFile,
  revokeThumbnailPreviewUrl,
} from '@/lib/playlist-thumbnail'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { api } from '@/lib/api'

type PlaylistFormMode = 'create' | 'edit'

interface PlaylistFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: PlaylistFormMode
  playlist?: CustomPlaylistSummary
}

export function PlaylistFormDialog({
  open,
  onOpenChange,
  mode,
  playlist,
}: PlaylistFormDialogProps) {
  const formId = useId()
  const fileInputRef = useRef<HTMLInputElement>(null)
  const data = usePlaylistsStore(state => state.data)
  const setData = usePlaylistsStore(state => state.setData)
  const setSelectedPlaylist = usePlaylistsStore(
    state => state.setSelectedPlaylist,
  )

  const isEdit = mode === 'edit'
  const existingThumbnail = usePlaylistThumbnail(
    isEdit ? playlist?.id : undefined,
    Boolean(playlist?.hasThumbnail),
  )

  const [name, setName] = useState(playlist?.name ?? '')
  const [nameError, setNameError] = useState<string | null>(null)
  const [thumbnailFile, setThumbnailFile] = useState<File | null>(null)
  const [thumbnailPreview, setThumbnailPreview] = useState<string | null>(null)
  const [removeThumbnail, setRemoveThumbnail] = useState(false)
  const [thumbnailError, setThumbnailError] = useState<string | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)

  useEffect(() => {
    if (!open) return
    // Opening the modal starts a fresh edit session from the selected playlist.
    /* eslint-disable react-hooks/set-state-in-effect -- Form draft state must reset when a new dialog session opens. */
    setName(playlist?.name ?? '')
    setNameError(null)
    setThumbnailFile(null)
    setThumbnailPreview(null)
    setRemoveThumbnail(false)
    setThumbnailError(null)
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [open, playlist])

  useEffect(() => {
    return () => {
      revokeThumbnailPreviewUrl(thumbnailPreview)
    }
  }, [thumbnailPreview])

  const previewSrc
    = thumbnailPreview ?? (!removeThumbnail ? existingThumbnail : null)

  const handleThumbnailChange = (file: File | null) => {
    setThumbnailError(null)
    setRemoveThumbnail(false)
    revokeThumbnailPreviewUrl(thumbnailPreview)

    if (!file) {
      setThumbnailFile(null)
      setThumbnailPreview(null)
      return
    }

    setThumbnailFile(file)
    setThumbnailPreview(createThumbnailPreviewUrl(file))
  }

  const handleRemoveThumbnail = () => {
    setThumbnailError(null)
    setThumbnailFile(null)
    setRemoveThumbnail(true)
    revokeThumbnailPreviewUrl(thumbnailPreview)
    setThumbnailPreview(null)
    if (fileInputRef.current) {
      fileInputRef.current.value = ''
    }
  }

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!data) return

    const trimmed = name.trim()
    if (trimmed.length === 0) {
      setNameError('Playlist name is required')
      return
    }
    if (trimmed.length > 100) {
      setNameError('Playlist name must be at most 100 characters')
      return
    }
    setNameError(null)

    let thumbnailPayload:
      | { data: string, mime: 'image/jpeg' | 'image/png' | 'image/webp' }
      | null
      | undefined

    try {
      if (thumbnailFile) {
        thumbnailPayload = await readPlaylistThumbnailFile(thumbnailFile)
      }
      else if (removeThumbnail) {
        thumbnailPayload = null
      }
    }
    catch (err) {
      setThumbnailError(
        err instanceof Error ? err.message : 'Invalid thumbnail',
      )
      return
    }

    setIsSubmitting(true)
    try {
      if (isEdit && playlist) {
        const updated = await api.updatePlaylist({
          playlistId: playlist.id,
          name: trimmed,
          thumbnailData: thumbnailPayload?.data ?? null,
          thumbnailMime: thumbnailPayload?.mime ?? null,
          clearThumbnail: thumbnailPayload === null,
        })
        setData({
          ...data,
          custom: data.custom.map(item =>
            item.id === updated.id ? updated : item,
          ),
        })
      }
      else {
        const created = await api.createPlaylist({
          name: trimmed,
          thumbnailData: thumbnailPayload?.data ?? null,
          thumbnailMime: thumbnailPayload?.mime ?? null,
        })
        setData({ ...data, custom: [...data.custom, created] })
        setSelectedPlaylist(created.id)
      }
      onOpenChange(false)
    }
    catch (err) {
      setThumbnailError(
        err instanceof Error ? err.message : 'Failed to save playlist',
      )
    }
    finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{isEdit ? 'Edit playlist' : 'Create playlist'}</DialogTitle>
          <DialogDescription>
            {isEdit
              ? 'Update the playlist name or cover image.'
              : 'Give your playlist a name and optional cover image.'}
          </DialogDescription>
        </DialogHeader>

        <form id={formId} onSubmit={handleSubmit} className="flex flex-col gap-5">
          <div className="flex flex-col gap-1.5">
            <label
              htmlFor={`${formId}-name`}
              className="text-sm font-medium text-foreground"
            >
              Name
            </label>
            <Input
              id={`${formId}-name`}
              value={name}
              onChange={event => setName(event.target.value)}
              aria-invalid={Boolean(nameError)}
              placeholder="My playlist"
              autoComplete="off"
            />
            {nameError
              ? (
                  <p className="text-xs text-destructive">{nameError}</p>
                )
              : null}
          </div>

          <div className="flex flex-col gap-1.5">
            <label
              htmlFor={`${formId}-thumbnail`}
              className="text-sm font-medium text-foreground"
            >
              Cover image
            </label>
            <div className="flex items-start gap-4">
              <div className="relative size-24 shrink-0 overflow-hidden rounded-lg bg-muted shadow-sm ring-1 ring-border/60">
                {previewSrc
                  ? (
                      <img
                        src={previewSrc}
                        alt="Playlist cover preview"
                        className="size-full object-cover"
                      />
                    )
                  : (
                      <div className="flex size-full items-center justify-center bg-linear-to-br from-slate-600 to-slate-800 text-white/80">
                        <ImagePlus className="size-8" />
                      </div>
                    )}
              </div>

              <div className="flex min-w-0 flex-1 flex-col gap-2">
                <Input
                  ref={fileInputRef}
                  id={`${formId}-thumbnail`}
                  type="file"
                  accept="image/jpeg,image/png,image/webp"
                  onChange={event =>
                    handleThumbnailChange(event.target.files?.[0] ?? null)}
                />
                <p className="text-xs text-muted-foreground">
                  JPEG, PNG, or WebP up to 512KB.
                </p>
                {previewSrc
                  ? (
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={handleRemoveThumbnail}
                        className="w-fit"
                      >
                        <Trash2 />
                        Remove image
                      </Button>
                    )
                  : null}
                {thumbnailError
                  ? (
                      <p className="text-xs text-destructive">{thumbnailError}</p>
                    )
                  : null}
              </div>
            </div>
          </div>
        </form>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={isSubmitting}
          >
            Cancel
          </Button>
          <Button type="submit" form={formId} disabled={isSubmitting}>
            {isSubmitting
              ? 'Saving...'
              : isEdit
                ? 'Save changes'
                : 'Create playlist'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
