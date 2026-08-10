import { useEffect, useId, useState } from 'react'
import { FileUp } from 'lucide-react'
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
  const data = usePlaylistsStore(state => state.data)
  const setData = usePlaylistsStore(state => state.setData)
  const setSelectedPlaylist = usePlaylistsStore(state => state.setSelectedPlaylist)

  const isEdit = mode === 'edit'

  const [createTab, setCreateTab] = useState<CreateTab>('new')
  const [name, setName] = useState(playlist?.name ?? '')
  const [nameError, setNameError] = useState<string | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)
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
    setSaveError(null)
    setImportPath(null)
    setImportPreview(null)
    setImportError(null)
    setImportAnalyzing(false)
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [open, playlist])

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

    setSaveError(null)
    setIsSubmitting(true)
    try {
      if (isEdit && playlist) {
        const updated = await api.updatePlaylist({
          playlistId: playlist.id,
          name: trimmed,
        })
        setData({
          ...data,
          custom: data.custom.map(item =>
            item.id === updated.id ? updated : item,
          ),
        })
      }
      else {
        const created = await api.createPlaylist({ name: trimmed })
        setData({ ...data, custom: [...data.custom, created] })
      }
      onOpenChange(false)
    }
    catch (err) {
      setSaveError(
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
              setSaveError(null)
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

      </FieldGroup>
      <FieldError>{saveError}</FieldError>
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
    ? 'Update the playlist name.'
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
