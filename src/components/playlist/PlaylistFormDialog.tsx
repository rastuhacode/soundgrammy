import { useEffect, useId, useRef, useState } from 'react'
import { FileUp, ImagePlus, Trash2 } from 'lucide-react'
import { open as openFileDialog } from '@tauri-apps/plugin-dialog'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from '@/components/ui/fieldset'
import { Input } from '@/components/ui/input'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { PlaylistImportMatchLists } from '@/components/playlist/PlaylistImportMatchLists'
import type { CustomPlaylistSummary } from '@/lib/db'
import { usePlaylistThumbnail } from '@/hooks/use-playlist-thumbnail'
import {
  createThumbnailPreviewUrl,
  readPlaylistThumbnailFile,
  revokeThumbnailPreviewUrl,
} from '@/lib/playlist-thumbnail'
import { fileBasename, validatePlaylistName } from '@/lib/playlist-form'
import { formatInvokeError } from '@/lib/playlist-recipe-io'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { api } from '@/lib/api'
import type { PlaylistImportPreview } from '@/types'

type PlaylistFormMode = 'create' | 'edit'
type CreateTab = 'new' | 'import'

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
  const setSelectedPlaylist = usePlaylistsStore(state => state.setSelectedPlaylist)

  const isEdit = mode === 'edit'
  const existingThumbnail = usePlaylistThumbnail(
    isEdit ? playlist?.id : undefined,
    Boolean(playlist?.hasThumbnail),
  )

  const [createTab, setCreateTab] = useState<CreateTab>('new')
  const [name, setName] = useState(playlist?.name ?? '')
  const [nameError, setNameError] = useState<string | null>(null)
  const [thumbnailFile, setThumbnailFile] = useState<File | null>(null)
  const [thumbnailPreview, setThumbnailPreview] = useState<string | null>(null)
  const [removeThumbnail, setRemoveThumbnail] = useState(false)
  const [thumbnailError, setThumbnailError] = useState<string | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)

  const [importPath, setImportPath] = useState<string | null>(null)
  const [importPreview, setImportPreview] = useState<PlaylistImportPreview | null>(null)
  const [importError, setImportError] = useState<string | null>(null)
  const [importAnalyzing, setImportAnalyzing] = useState(false)

  useEffect(() => {
    if (!open) return
    // Opening the modal starts a fresh edit session from the selected playlist.
    /* eslint-disable react-hooks/set-state-in-effect -- Form draft state must reset when a new dialog session opens. */
    setCreateTab('new')
    setName(playlist?.name ?? '')
    setNameError(null)
    setThumbnailFile(null)
    setThumbnailPreview(null)
    setRemoveThumbnail(false)
    setThumbnailError(null)
    setImportPath(null)
    setImportPreview(null)
    setImportError(null)
    setImportAnalyzing(false)
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

  const handleSelectImportFile = async () => {
    if (importAnalyzing || isSubmitting) return
    try {
      const path = await openFileDialog({
        multiple: false,
        filters: [
          {
            name: 'SoundGrammy playlist',
            extensions: ['json'],
          },
        ],
      })
      if (!path || Array.isArray(path)) return

      setImportAnalyzing(true)
      setImportError(null)
      setImportPreview(null)
      setImportPath(path)
      const preview = await api.analyzePlaylistJson(path)
      setImportPreview(preview)
      setName(preview.suggestedName)
      setNameError(null)
    }
    catch (error) {
      setImportPreview(null)
      setImportError(formatInvokeError(error))
    }
    finally {
      setImportAnalyzing(false)
    }
  }

  const handleClearImportFile = () => {
    setImportPath(null)
    setImportPreview(null)
    setImportError(null)
    setName('')
    setNameError(null)
  }

  const handleCreateSubmit = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!data) return

    const nameIssue = validatePlaylistName(name)
    if (nameIssue) {
      setNameError(nameIssue)
      return
    }
    setNameError(null)
    const trimmed = name.trim()

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
      return setThumbnailError(
        err instanceof Error ? err.message : 'Invalid thumbnail',
      )
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

  const handleImportCreate = async () => {
    if (!data || !importPath || !importPreview || isSubmitting || importAnalyzing) return

    const nameIssue = validatePlaylistName(name)
    if (nameIssue) {
      setNameError(nameIssue)
      return
    }
    setNameError(null)

    setIsSubmitting(true)
    setImportError(null)
    try {
      const result = await api.importPlaylistJson(importPath, name.trim())
      const playlists = await api.listPlaylists()
      setData(playlists)
      setSelectedPlaylist(result.playlistId)
      onOpenChange(false)
    }
    catch (error) {
      setImportError(formatInvokeError(error))
    }
    finally {
      setIsSubmitting(false)
    }
  }

  const newForm = (
    <form id={formId} onSubmit={handleCreateSubmit} className="flex flex-col gap-5" noValidate>
      <FieldGroup>
        <Field data-invalid={nameError ? true : undefined}>
          <FieldLabel htmlFor={`${formId}-name`}>Name</FieldLabel>
          <Input
            id={`${formId}-name`}
            value={name}
            onChange={(event) => {
              setNameError(null)
              setName(event.target.value)
            }}
            aria-invalid={nameError ? true : undefined}
            placeholder="My playlist"
            autoComplete="off"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck={false}
          />
          <FieldError>{nameError}</FieldError>
        </Field>

        <Field data-invalid={thumbnailError ? true : undefined}>
          <FieldLabel htmlFor={`${formId}-thumbnail`}>Cover image</FieldLabel>
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

            <div className="flex min-w-0 grow flex-col gap-2">
              <input
                ref={fileInputRef}
                id={`${formId}-thumbnail`}
                type="file"
                accept="image/jpeg,image/png,image/webp"
                className="sr-only"
                onChange={event =>
                  handleThumbnailChange(event.target.files?.[0] ?? null)}
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="w-fit"
                onClick={() => fileInputRef.current?.click()}
              >
                <ImagePlus />
                {thumbnailFile ? 'Change image' : 'Choose image'}
              </Button>
              <FieldDescription className="text-xs">
                JPEG, PNG, or WebP up to 512KB.
              </FieldDescription>
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
              <FieldError>{thumbnailError}</FieldError>
            </div>
          </div>
        </Field>
      </FieldGroup>
    </form>
  )

  const importForm = (
    <FieldGroup>
      <Field data-invalid={importError ? true : undefined}>
        <FieldLabel>Playlist file</FieldLabel>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            type="button"
            variant="outline"
            onClick={() => void handleSelectImportFile()}
            disabled={importAnalyzing || isSubmitting}
          >
            <FileUp />
            {importPath ? 'Choose another file' : 'Choose JSON file'}
          </Button>
          {importPath
            ? (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={handleClearImportFile}
                  disabled={importAnalyzing || isSubmitting}
                >
                  Clear
                </Button>
              )
            : null}
        </div>
        {importPath
          ? (
              <FieldDescription className="truncate text-xs" title={importPath}>
                {fileBasename(importPath)}
              </FieldDescription>
            )
          : (
              <FieldDescription className="text-xs">
                Select a `.soundgrammy.json` export from this Telegram account.
              </FieldDescription>
            )}
        {importAnalyzing
          ? (
              <FieldDescription className="text-xs">Analyzing file…</FieldDescription>
            )
          : null}
        <FieldError className="whitespace-pre-wrap">{importError}</FieldError>
      </Field>

      {importPreview
        ? (
            <>
              <Field data-invalid={nameError ? true : undefined}>
                <FieldLabel htmlFor={`${formId}-import-name`}>Name</FieldLabel>
                <Input
                  id={`${formId}-import-name`}
                  value={name}
                  onChange={(event) => {
                    setNameError(null)
                    setName(event.target.value)
                  }}
                  aria-invalid={nameError ? true : undefined}
                  placeholder="Playlist name"
                  autoComplete="off"
                  autoCorrect="off"
                  autoCapitalize="off"
                  spellCheck={false}
                />
                {nameError
                  ? <FieldError>{nameError}</FieldError>
                  : (
                      <FieldDescription className="text-xs">
                        Filled from the file; you can change it before creating.
                      </FieldDescription>
                    )}
              </Field>

              <PlaylistImportMatchLists
                succeeded={importPreview.succeeded}
                failed={importPreview.failed}
              />
            </>
          )
        : null}
    </FieldGroup>
  )

  const dialogTitle = isEdit
    ? 'Edit playlist'
    : createTab === 'import'
      ? 'Import playlist'
      : 'Create playlist'

  const dialogDescription = isEdit
    ? 'Update the playlist name or cover image.'
    : createTab === 'import'
      ? 'Import playlist from file. Keep in mind that it is not cross-user operation.'
      : 'Create a playlist from scratch.'

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{dialogTitle}</DialogTitle>
          <DialogDescription>{dialogDescription}</DialogDescription>
        </DialogHeader>

        {isEdit
          ? newForm
          : (
              <Tabs
                value={createTab}
                onValueChange={(value) => {
                  if (value === 'new' || value === 'import') setCreateTab(value)
                }}
                className="gap-4"
              >
                <TabsList className="w-full">
                  <TabsTrigger value="new" className="flex-1">
                    New
                  </TabsTrigger>
                  <TabsTrigger value="import" className="flex-1">
                    Import
                  </TabsTrigger>
                </TabsList>
                <TabsContent value="new">{newForm}</TabsContent>
                <TabsContent value="import">{importForm}</TabsContent>
              </Tabs>
            )}

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={isSubmitting}
          >
            Cancel
          </Button>
          {isEdit || createTab === 'new'
            ? (
                <Button type="submit" form={formId} disabled={isSubmitting}>
                  {isSubmitting
                    ? 'Saving...'
                    : isEdit
                      ? 'Save changes'
                      : 'Create playlist'}
                </Button>
              )
            : (
                <Button
                  type="button"
                  onClick={() => void handleImportCreate()}
                  disabled={
                    isSubmitting
                    || importAnalyzing
                    || !importPreview
                    || importPreview.succeeded.length === 0
                  }
                >
                  {isSubmitting ? 'Creating…' : 'Create playlist'}
                </Button>
              )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
