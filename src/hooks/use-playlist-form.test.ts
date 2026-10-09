// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { open as openFileDialog } from '@tauri-apps/plugin-dialog'
import { api } from '@/lib/api'
import { usePlaylistForm, type PlaylistFormOptions } from './use-playlist-form'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useSessionStore } from '@/stores/session-store'
import { deferred, playlists } from '@/test-support/fixtures'
import type { CustomPlaylistSummary, PlaylistImportPreview } from '@/types'

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('@/lib/api', () => ({ api: { analyzePlaylistJson: vi.fn() } }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
const preview: PlaylistImportPreview = { suggestedName: 'Old import', succeeded: [], failed: [] }
beforeEach(() => {
  vi.restoreAllMocks()
  vi.clearAllMocks()
  useSessionStore.getState().clearSession()
  usePlaylistsStore.getState().hydrate(playlists())
})

async function mountForm(initial: Partial<PlaylistFormOptions> = {}) {
  let form!: ReturnType<typeof usePlaylistForm>
  const options: PlaylistFormOptions = { open: true, mode: 'create', onOpenChange: vi.fn(), ...initial }
  function Probe(props: PlaylistFormOptions) {
    form = usePlaylistForm(props)
    return null
  }
  const root = createRoot(document.createElement('div'))
  const render = async (open: boolean) => {
    options.open = open
    await act(async () => root.render(createElement(Probe, { ...options })))
  }
  await render(true)
  return { get form() {
    return form
  }, options, render, root }
}

describe('playlist dialog operation lifetime', () => {
  it('discards analysis from a closed/reopened dialog while a new analysis remains busy', async () => {
    const oldAnalysis = deferred<PlaylistImportPreview>()
    const newAnalysis = deferred<PlaylistImportPreview>()
    vi.mocked(openFileDialog).mockResolvedValue('/old.json')
    vi.mocked(api.analyzePlaylistJson).mockReturnValueOnce(oldAnalysis.promise).mockReturnValueOnce(newAnalysis.promise)
    const h = await mountForm()
    let oldRequest!: Promise<void>
    let newRequest!: Promise<void>
    try {
      await act(async () => {
        oldRequest = h.form.handleSelectImportFile()
      })
      await act(async () => h.form.handleOpenChange(false))
      await h.render(false)
      await h.render(true)
      vi.mocked(openFileDialog).mockResolvedValue('/new.json')
      await act(async () => {
        newRequest = h.form.handleSelectImportFile()
      })
      await act(async () => {
        oldAnalysis.resolve(preview)
        await oldRequest
      })
      expect(h.form.importPreview).toBeNull()
      expect(h.form.name).toBe('')
      expect(h.form.importAnalyzing).toBe(true)
      expect(h.form.importPath).toBe('/new.json')
      await act(async () => {
        newAnalysis.resolve({ ...preview, suggestedName: 'New import' })
        await newRequest
      })
      expect(h.form.name).toBe('New import')
      expect(h.form.importAnalyzing).toBe(false)
    }
    finally { await act(async () => h.root.unmount()) }
  })

  it('does not analyze a file selected after its dialog was closed', async () => {
    const picker = deferred<string | null>()
    vi.mocked(openFileDialog).mockReturnValueOnce(picker.promise)
    const h = await mountForm()
    let request!: Promise<void>
    try {
      await act(async () => {
        request = h.form.handleSelectImportFile()
      })
      await h.render(false)
      await h.render(true)
      await act(async () => {
        picker.resolve('/old.json')
        await request
      })
      expect(api.analyzePlaylistJson).not.toHaveBeenCalled()
      expect(h.form.importPath).toBeNull()
    }
    finally { await act(async () => h.root.unmount()) }
  })

  it('does not close a reopened dialog when an earlier save finishes', async () => {
    const save = deferred<CustomPlaylistSummary>()
    vi.spyOn(usePlaylistsStore.getState(), 'createPlaylist').mockReturnValueOnce(save.promise)
    const h = await mountForm()
    let request!: Promise<void>
    try {
      await act(async () => h.form.setName('Old draft'))
      await act(async () => {
        request = h.form.handleCreateSubmit({ preventDefault: vi.fn() } as unknown as React.FormEvent)
      })
      await h.render(false)
      await h.render(true)
      await act(async () => h.form.setName('New draft'))
      await act(async () => {
        save.resolve({ id: 30, name: 'Old draft', trackIds: [], updatedAt: '' })
        await request
      })
      expect(h.options.onOpenChange).not.toHaveBeenCalled()
      expect(h.form.name).toBe('New draft')
      expect(h.form.isSubmitting).toBe(false)
    }
    finally { await act(async () => h.root.unmount()) }
  })

  it('closes after saving even if the same playlist receives its updated name first', async () => {
    const save = deferred<void>()
    vi.spyOn(usePlaylistsStore.getState(), 'updatePlaylist').mockReturnValueOnce(save.promise)
    const playlist: CustomPlaylistSummary = { id: 30, name: 'Old name', trackIds: [], updatedAt: '' }
    const h = await mountForm({ mode: 'edit', playlist })
    let request!: Promise<void>
    try {
      await act(async () => h.form.setName('New name'))
      await act(async () => {
        request = h.form.handleCreateSubmit({ preventDefault: vi.fn() } as unknown as React.FormEvent)
      })
      h.options.playlist = { ...playlist, name: 'New name' }
      await h.render(true)
      await act(async () => {
        save.resolve()
        await request
      })
      expect(h.options.onOpenChange).toHaveBeenCalledWith(false)
    }
    finally { await act(async () => h.root.unmount()) }
  })

  it('discards a save completion after switching the dialog to a different playlist', async () => {
    const save = deferred<void>()
    vi.spyOn(usePlaylistsStore.getState(), 'updatePlaylist').mockReturnValueOnce(save.promise)
    const playlist: CustomPlaylistSummary = { id: 30, name: 'Old name', trackIds: [], updatedAt: '' }
    const h = await mountForm({ mode: 'edit', playlist })
    let request!: Promise<void>
    try {
      await act(async () => {
        request = h.form.handleCreateSubmit({ preventDefault: vi.fn() } as unknown as React.FormEvent)
      })
      h.options.playlist = { ...playlist, id: 31, name: 'Other playlist' }
      await h.render(true)
      await act(async () => {
        save.resolve()
        await request
      })
      expect(h.options.onOpenChange).not.toHaveBeenCalled()
      expect(h.form.name).toBe('Other playlist')
    }
    finally { await act(async () => h.root.unmount()) }
  })

  it('discards account-scoped analysis after logout', async () => {
    const pending = deferred<PlaylistImportPreview>()
    vi.mocked(openFileDialog).mockResolvedValueOnce('/old.json')
    vi.mocked(api.analyzePlaylistJson).mockReturnValueOnce(pending.promise)
    const h = await mountForm()
    let request!: Promise<void>
    try {
      await act(async () => {
        request = h.form.handleSelectImportFile()
      })
      useSessionStore.getState().clearSession()
      await act(async () => {
        pending.resolve(preview)
        await request
      })
      expect(h.form.importPreview).toBeNull()
      expect(h.form.name).toBe('')
    }
    finally { await act(async () => h.root.unmount()) }
  })
})
