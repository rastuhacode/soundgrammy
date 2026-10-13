// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { describe, expect, it, vi } from 'vitest'
import { usePlaylistView } from './use-playlist-view'
import { useLibraryStore } from '@/stores/library-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import { usePlaylistJobsStore } from '@/stores/playlist-jobs-store'
import { sendPlayerCommand } from '@/stores/player-store'
import { useSessionStore } from '@/stores/session-store'
import { api } from '@/lib/api'
import { libraryPage, playlists, track } from '@/test-support/fixtures'

vi.mock('@/lib/api', () => ({ api: { listTracksPage: vi.fn(), nativePlayerCommand: vi.fn(async () => undefined), nativeAudioSnapshot: vi.fn(async () => undefined) } }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
describe('membership-scoped selection', () => {
  it('keeps search, duplicate playlist memberships, smart playlists and whole-library playback beyond page boundaries', async () => {
    vi.clearAllMocks()
    useSessionStore.getState().clearSession()
    useListenStatsStore.getState().reset()
    const tracks = Array.from({ length: 10_037 }, (_, index) => track(index + 1))
    vi.mocked(api.listTracksPage).mockImplementation(async (offset = 0) =>
      libraryPage(tracks.slice(offset, offset + 500), offset, offset + 500 < tracks.length ? offset + 500 : null, 7))
    await useLibraryStore.getState().refresh()
    usePlaylistsStore.getState().hydrate(playlists([1, 10_037, 10_037]))
    usePlaylistsStore.getState().setSelectedPlaylist(20)
    let view!: ReturnType<typeof usePlaylistView>
    function Probe() {
      view = usePlaylistView()
      return null
    }
    const root = createRoot(document.createElement('div'))
    const download = vi.spyOn(usePlaylistJobsStore.getState(), 'runDownloadPlaylist').mockResolvedValue(undefined)
    const cache = vi.spyOn(usePlaylistJobsStore.getState(), 'runCachePlaylist').mockResolvedValue(undefined)
    try {
      await act(async () => root.render(createElement(Probe)))
      await act(async () => view.search.setValue('Track 10037'))
      expect(view.table.entries.map(entry => entry.sourceIndex)).toEqual([1, 2])
      await act(async () => {
        view.playbackActions.onPlay()
        await sendPlayerCommand({ type: 'attach' })
      })
      expect(api.nativePlayerCommand).toHaveBeenCalledWith(expect.objectContaining({
        type: 'playPlaylist', queue: expect.objectContaining({ tracks: [track(1), track(10_037), track(10_037)] }),
      }))
      await act(async () => {
        view.search.setValue('')
        usePlaylistsStore.getState().setSelectedPlaylist('all')
      })
      await act(async () => {
        view.playbackActions.onShuffle()
        await sendPlayerCommand({ type: 'attach' })
      })
      const command = vi.mocked(api.nativePlayerCommand).mock.calls.map(([command]) => command)
        .reverse().find(command => command.type === 'playPlaylist')
      expect(command?.type === 'playPlaylist' && command.queue.tracks.length).toBe(10_037)
      expect(command?.type === 'playPlaylist' && command.queue.tracks.at(-1)?.id).toBe(10_037)
      await act(async () => {
        await view.playbackActions.onDownloadPlaylist()
        await view.playbackActions.onCachePlaylist()
      })
      const allIds = tracks.map(track => track.id)
      expect(download).toHaveBeenCalledExactlyOnceWith({ playlistId: 'all', name: 'All tracks', trackIds: allIds })
      expect(cache).toHaveBeenCalledExactlyOnceWith({ playlistId: 'all', name: 'All tracks', trackIds: allIds })
      await act(async () => {
        useListenStatsStore.getState().upsert({ track_id: 10_037, starts: 1, qualified_plays: 1,
          completes: 1, early_skips: 0, total_listened_ms: 120000,
          first_played_at_ms: 1, last_played_at_ms: 2, likeness: 1 })
        usePlaylistsStore.getState().setSelectedPlaylist('popular')
      })
      expect(view.table.entries.map(entry => entry.track.id)).toEqual([10_037])
    }
    finally {
      await act(async () => root.unmount())
      download.mockRestore()
      cache.mockRestore()
    }
  })
  it('shares multi-column order between displayed memberships and playback, including search', async () => {
    vi.clearAllMocks()
    const first = { ...track(1), title: 'Same', performer: 'Zed' }
    const second = { ...track(2), title: 'Same', performer: 'Ann' }
    const third = { ...track(3), title: 'Other', performer: null }
    useLibraryStore.getState().setLibrary([first, second, third])
    usePlaylistsStore.getState().hydrate(playlists([1, 2, 1, 3]))
    usePlaylistsStore.getState().setSelectedPlaylist(20)
    let view!: ReturnType<typeof usePlaylistView>
    function Probe() {
      view = usePlaylistView()
      return null
    }
    const root = createRoot(document.createElement('div'))
    try {
      await act(async () => root.render(createElement(Probe)))
      await act(async () => view.table.onSortingChange([
        { id: 'title', desc: true }, { id: 'performer', desc: false },
      ]))
      expect(view.table.entries.map(entry => entry.sourceIndex)).toEqual([1, 0, 2, 3])
      await act(async () => view.search.setValue('Same'))
      expect(view.table.entries.map(entry => entry.sourceIndex)).toEqual([1, 0, 2])
      await act(async () => {
        view.table.onTrackPlay(first, 2)
        await sendPlayerCommand({ type: 'attach' })
      })
      expect(api.nativePlayerCommand).toHaveBeenCalledWith(expect.objectContaining({
        type: 'playPlaylist', queue: expect.objectContaining({
          cursor: 2, sourceIndices: [1, 0, 2, 3], tracks: [second, first, first, third],
        }),
      }))
    }
    finally { await act(async () => root.unmount()) }
  })
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
