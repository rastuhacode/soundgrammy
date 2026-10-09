// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { expect, it, vi } from 'vitest'
import { PlaylistTracksTable } from './PlaylistTracksTable'
import { usePlaylistView } from '@/hooks/use-playlist-view'
import { useLibraryStore } from '@/stores/library-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { sendPlayerCommand } from '@/stores/player-store'
import { api } from '@/lib/api'
import { playlists, track } from '@/test-support/fixtures'

vi.mock('@/lib/api', () => ({ api: {
  nativePlayerCommand: vi.fn(async () => undefined), nativeAudioSnapshot: vi.fn(async () => undefined),
} }))
vi.mock('./PlaylistTrackThumbnail', () => ({ TrackThumbnail: () => null }))
vi.mock('@/hooks/use-touch-screen', () => ({ useTouchScreen: () => false }))
vi.mock('@/hooks/use-compact-display', () => ({ useCompactDisplay: () => ({ isCompact: false }) }))
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    measure: vi.fn(), getTotalSize: () => count * 78,
    getVirtualItems: () => Array.from({ length: count }, (_, index) => ({ index, start: index * 78 })),
  }),
}))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

it('renders the same multi-column order it sends to native playback after shift-click sorting', async () => {
  const first = { ...track(1), title: 'Same', performer: 'Zed' }
  const second = { ...track(2), title: 'Same', performer: 'Ann' }
  const third = { ...track(3), title: 'Other' }
  useLibraryStore.getState().setLibrary([first, second, third])
  usePlaylistsStore.getState().hydrate(playlists([1, 2, 1, 3]))
  usePlaylistsStore.getState().setSelectedPlaylist(20)
  function Screen() {
    const view = usePlaylistView()
    return createElement(PlaylistTracksTable, view.table)
  }
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  try {
    await act(async () => root.render(createElement(Screen)))
    const headerButtons = host.querySelectorAll('[role="columnheader"] button')
    await act(async () => headerButtons[0].dispatchEvent(new MouseEvent('click', { bubbles: true })))
    await act(async () => headerButtons[1].dispatchEvent(new MouseEvent('click', { bubbles: true, shiftKey: true })))
    const rows = host.querySelectorAll('[role="row"][aria-selected]')
    expect(rows[0].textContent).toContain('Other')
    expect(rows[1].textContent).toContain('Ann')
    expect(rows[2].textContent).toContain('Zed')
    expect(rows[3].textContent).toContain('Zed')
    await act(async () => {
      rows[3].dispatchEvent(new MouseEvent('click', { bubbles: true }))
      await sendPlayerCommand({ type: 'attach' })
    })
    expect(api.nativePlayerCommand).toHaveBeenCalledWith(expect.objectContaining({
      type: 'playPlaylist', queue: expect.objectContaining({
        cursor: 3, sourceIndices: [3, 1, 0, 2], tracks: [third, second, first, first],
      }),
    }))
  }
  finally {
    await act(async () => root.unmount())
    host.remove()
  }
})
