import { useEffect, useState } from 'react'
import { open as openFileDialog } from '@tauri-apps/plugin-dialog'
import type { CustomPlaylistSummary, PlaylistImportPreview } from '@/types'
import { validatePlaylistName } from '@/lib/playlist-form'
import { formatInvokeError } from '@/lib/playlist-recipe-io'
import { api } from '@/lib/api'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useAsyncScope } from '@/hooks/use-async-scope'

type CreateTab = 'new' | 'import'

export interface PlaylistFormOptions {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: 'create' | 'edit'
  playlist?: CustomPlaylistSummary
  trackIds?: number[]
}

/** Own the playlist draft, import preparation, and completion lifetime. */
export function usePlaylistForm({ open, onOpenChange, mode, playlist, trackIds }: PlaylistFormOptions) {
  const data = usePlaylistsStore(state => state.data)
  const createPlaylist = usePlaylistsStore(state => state.createPlaylist)
  const updatePlaylist = usePlaylistsStore(state => state.updatePlaylist)
  const importPlaylist = usePlaylistsStore(state => state.importPlaylist)
  const setSelectedPlaylist = usePlaylistsStore(state => state.setSelectedPlaylist)

  const isEdit = mode === 'edit'
  const { capture, invalidate } = useAsyncScope(open, `${mode}:${playlist?.id ?? ''}`)
  const handleOpenChange = (next: boolean) => {
    if (!next) invalidate()
    onOpenChange(next)
  }

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
    setIsSubmitting(false)
    setImportPath(null)
    setImportPreview(null)
    setImportError(null)
    setImportAnalyzing(false)
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [open, mode, playlist])

  const handleSelectImportFile = async () => {
    if (importAnalyzing || isSubmitting || !open) return
    const current = capture()
    setImportAnalyzing(true)
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
      if (!current() || !path || Array.isArray(path)) return

      setImportError(null)
      setImportPreview(null)
      setImportPath(path)
      const preview = await api.analyzePlaylistJson(path)
      if (!current()) return
      setImportPreview(preview)
      setName(preview.suggestedName)
      setNameError(null)
    }
    catch (error) {
      if (!current()) return
      setImportPreview(null)
      setImportError(formatInvokeError(error))
    }
    finally {
      if (current()) setImportAnalyzing(false)
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
    if (!data || !open || isSubmitting) return
    const current = capture()

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
        await updatePlaylist(playlist.id, trimmed)
      }
      else {
        await createPlaylist(trimmed, trackIds)
      }
      if (current()) handleOpenChange(false)
    }
    catch (err) {
      if (!current()) return
      setSaveError(
        formatInvokeError(err),
      )
    }
    finally {
      if (current()) setIsSubmitting(false)
    }
  }

  const handleImportCreate = async () => {
    if (!data || !open || !importPath || !importPreview || isSubmitting || importAnalyzing) return
    const current = capture()

    const nameIssue = validatePlaylistName(name)
    if (nameIssue) {
      setNameError(nameIssue)
      return
    }
    setNameError(null)

    setIsSubmitting(true)
    setImportError(null)
    try {
      const result = await importPlaylist(importPath, name.trim())
      if (!current()) return
      setSelectedPlaylist(result.playlistId)
      if (current()) handleOpenChange(false)
    }
    catch (error) {
      if (!current()) return
      setImportError(formatInvokeError(error))
    }
    finally {
      if (current()) setIsSubmitting(false)
    }
  }

  return {
    isEdit, createTab, setCreateTab, name, setName, nameError, setNameError,
    saveError, setSaveError, isSubmitting, importPath, importPreview, importError,
    importAnalyzing, handleSelectImportFile, handleClearImportFile,
    handleCreateSubmit, handleImportCreate, handleOpenChange,
  }
}
