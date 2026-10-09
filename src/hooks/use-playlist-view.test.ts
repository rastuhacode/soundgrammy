// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { describe, expect, it, vi } from 'vitest'
import { usePlaylistView } from './use-playlist-view'
import { useLibraryStore } from '@/stores/library-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { sendPlayerCommand } from '@/stores/player-store'
import { useSessionStore } from '@/stores/session-store'
import { api } from '@/lib/api'
import { playlists, track } from '@/test-support/fixtures'

vi.mock('@/lib/api', () => ({ api: { nativePlayerCommand: vi.fn(async () => undefined), nativeAudioSnapshot: vi.fn(async () => undefined) } }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
describe('membership-scoped selection', () => {
  it('preserves duplicate membership positions through search, sort, and playback', async () => {
    vi.clearAllMocks()
    useSessionStore.getState().clearSession()
    useLibraryStore.getState().setLibrary([track(1), track(2)])
    usePlaylistsStore.getState().hydrate(playlists([1, 2, 1]))
    usePlaylistsStore.getState().setSelectedPlaylist(20)
    let view!: ReturnType<typeof usePlaylistView>
    function Probe() {
      view = usePlaylistView()
      return null
    }
    const root = createRoot(document.createElement('div'))
    try {
      await act(async () => root.render(createElement(Probe)))
      await act(async () => {
        view.search.setValue('Track 1')
        view.table.onSortingChange([{ id: 'title', desc: true }])
      })
      expect(view.table.entries.map(entry => entry.sourceIndex)).toEqual([0, 2])
      await act(async () => view.selection.enter(2))
      expect(view.selection.positions).toEqual([2])
      expect(view.selection.trackIds).toEqual([1])
      await act(async () => {
        view.table.onTrackPlay(track(1), 2)
        await sendPlayerCommand({ type: 'attach' })
      })
      expect(api.nativePlayerCommand).toHaveBeenCalledWith(expect.objectContaining({
        type: 'playPlaylist', queue: expect.objectContaining({ cursor: 2, sourceIndices: [1, 0, 2], tracks: [track(2), track(1), track(1)] }),
      }))
    }
    finally { await act(async () => root.unmount()) }
  })
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
      await act(async () => view.selection.enter(1))
      expect(view.selection.trackIds).toEqual([2])
      await act(async () => usePlaylistsStore.getState().setData(playlists([2, 3])))
      expect(view.selection.mode).toBe(false)
      expect(view.selection.trackIds).toEqual([])
      // Restoring the old order must not resurrect its stale selection.
      await act(async () => usePlaylistsStore.getState().setData(playlists()))
      expect(view.selection.trackIds).toEqual([])
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
      expect(view.table.entries.map(entry => entry.sourceIndex)).toEqual([1, 2])
      expect(view.table.canReorder).toBe(false)
      await act(async () => view.selection.enter(2))
      expect(view.selection.trackIds).toEqual([3])
    }
    finally {
      await act(async () => root.unmount())
    }
  })
})
