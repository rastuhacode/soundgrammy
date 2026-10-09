import { useId } from 'react'
import { FileUp } from 'lucide-react'
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
import { usePlaylistForm } from '@/hooks/use-playlist-form'
import { fileBasename } from '@/lib/playlist-form'

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
  const {
    isEdit, createTab, setCreateTab, name, setName, nameError, setNameError,
    saveError, setSaveError, isSubmitting, importPath, importPreview, importError,
    importAnalyzing, handleSelectImportFile, handleClearImportFile,
    handleCreateSubmit, handleImportCreate, handleOpenChange,
  } = usePlaylistForm({ open, onOpenChange, mode, playlist })

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
            autoFocus
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
            onClick={handleSelectImportFile}
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
    <Dialog open={open} onOpenChange={handleOpenChange}>
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
                  <TabsTrigger value="new" className="grow">
                    New
                  </TabsTrigger>
                  <TabsTrigger value="import" className="grow">
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
            onClick={() => handleOpenChange(false)}
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
                  onClick={handleImportCreate}
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
