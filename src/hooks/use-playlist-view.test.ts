// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { describe, expect, it, vi } from 'vitest'
import { usePlaylistView } from './use-playlist-view'
import { useLibraryStore } from '@/stores/library-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { playlists, track } from '@/test-support/fixtures'

vi.mock('@/lib/api', () => ({ api: {} }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
describe('membership-scoped selection', () => {
  it('clears selection before an earlier liked membership shifts its target', async () => {
    useLibraryStore.getState().setLibrary([track(1), track(2), track(3)])
    useListenStatsStore.getState().reset()
    usePlaylistsStore.getState().hydrate(playlists())
    usePlaylistsStore.getState().setSelectedPlaylist('liked')
    let view!: ReturnType<typeof usePlaylistView>
    function Probe() {
      view = usePlaylistView()
      return null
    }
    const host = document.createElement('div')
    const root = createRoot(host)
    try {
      await act(async () => root.render(createElement(Probe)))
      await act(async () => view.handleEnterSelection(1))
      expect(view.selectedTrackIds).toEqual([2])
      await act(async () => usePlaylistsStore.getState().setData(playlists([2, 3])))
      expect(view.selectionMode).toBe(false)
      expect(view.selectedTrackIds).toEqual([])
      // Restoring the old order must not resurrect its stale selection.
      await act(async () => usePlaylistsStore.getState().setData(playlists()))
      expect(view.selectedTrackIds).toEqual([])
    }
    finally {
      await act(async () => root.unmount())
    }
  })
  it('keeps backend membership positions when a library track is missing', async () => {
    useLibraryStore.getState().setLibrary([track(2), track(3)])
    usePlaylistsStore.getState().hydrate(playlists())
    usePlaylistsStore.getState().setSelectedPlaylist(20)
    let view!: ReturnType<typeof usePlaylistView>
    function Probe() {
      view = usePlaylistView()
      return null
    }
    const root = createRoot(document.createElement('div'))
    try {
      await act(async () => root.render(createElement(Probe)))
      expect(view.filteredSourceIndices).toEqual([1, 2])
      expect(view.canReorder).toBe(false)
      await act(async () => view.handleEnterSelection(2))
      expect(view.selectedTrackIds).toEqual([3])
    }
    finally {
      await act(async () => root.unmount())
    }
  })
})
