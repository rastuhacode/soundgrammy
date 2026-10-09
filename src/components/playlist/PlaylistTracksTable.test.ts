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

async function withSelectionTable(run: (screen: {
  view: () => ReturnType<typeof usePlaylistView>
  click: (index: number, shift?: boolean, checkbox?: boolean) => Promise<void>
}) => Promise<void>) {
  useLibraryStore.getState().setLibrary([track(1), track(2), track(3), track(4)])
  usePlaylistsStore.getState().hydrate(playlists([1, 2, 1, 3, 4]))
  usePlaylistsStore.getState().setSelectedPlaylist(20)
  let view!: ReturnType<typeof usePlaylistView>
  function Screen() {
    view = usePlaylistView()
    return createElement(PlaylistTracksTable, view.table)
  }
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  try {
    await act(async () => root.render(createElement(Screen)))
    await act(async () => view.selection.enter(0))
    await run({
      view: () => view,
      click: async (index, shift = false, checkbox = false) => {
        const row = host.querySelectorAll('[role="row"][aria-selected]')[index]!
        const target = checkbox ? row.querySelector('[role="checkbox"]')! : row
        await act(async () => target.dispatchEvent(new MouseEvent('click', { bubbles: true, shiftKey: shift })))
      },
    })
  }
  finally {
    await act(async () => root.unmount())
    host.remove()
  }
}

it('adds inclusive forward and backward ranges from the last selected row', async () => {
  await withSelectionTable(async ({ view, click }) => {
    await click(2, true)
    expect(view().selection.positions).toEqual([0, 1, 2])
    // Ordinary selections update the anchor while preserving existing selections.
    await click(4)
    await click(3, true)
    expect(view().selection.positions).toEqual([0, 1, 2, 3, 4])
    await click(1)
    expect(view().selection.positions).toEqual([0, 2, 3, 4])
    // A selected endpoint is still selected by Shift-click.
    await click(0, true)
    expect(view().selection.positions).toEqual([0, 1, 2, 3, 4])
  })
})

it('uses the endpoint of the previous range as the next anchor', async () => {
  await withSelectionTable(async ({ view, click }) => {
    await click(4, true)
    await click(1)
    await click(2, true)
    expect(view().selection.positions).toEqual([0, 2, 3, 4])
  })
})

it('supports range selection from checkbox clicks', async () => {
  await withSelectionTable(async ({ view, click }) => {
    await click(1, false, true)
    await click(4, true, true)
    expect(view().selection.positions).toEqual([0, 1, 2, 3, 4])
  })
})

it('uses displayed order for sorted duplicate tracks and skips filtered tracks', async () => {
  await withSelectionTable(async ({ view, click }) => {
    await act(async () => view().table.onSortingChange([{ id: 'title', desc: false }]))
    expect(view().table.entries.map(entry => entry.sourceIndex)).toEqual([0, 2, 1, 3, 4])
    await click(1, true)
    expect(view().selection.positions).toEqual([0, 2])
    await act(async () => {
      view().selection.exit()
      view().search.setValue('Track 1')
    })
    await act(async () => view().selection.enter(0))
    await click(1, true)
    expect(view().selection.positions).toEqual([0, 2])
  })
})

it('selects only the clicked track when the anchor is hidden or selection was cleared', async () => {
  await withSelectionTable(async ({ view, click }) => {
    await act(async () => view().search.setValue('Track 4'))
    await click(0, true)
    expect(view().selection.positions).toEqual([0, 4])
    await act(async () => {
      view().search.setValue('')
      view().selection.clear()
    })
    await click(3, true)
    expect(view().selection.positions).toEqual([3])
    await click(3)
    await click(1, true)
    expect(view().selection.positions).toEqual([1])
  })
})

it('resets the range anchor after leaving selection or changing membership', async () => {
  await withSelectionTable(async ({ view, click }) => {
    await click(4)
    await act(async () => view().selection.exit())
    await act(async () => view().selection.enter(1))
    await click(2, true)
    expect(view().selection.positions).toEqual([1, 2])
    await act(async () => usePlaylistsStore.getState().setData(playlists([4, 3, 1, 2, 1])))
    expect(view().selection.mode).toBe(false)
    await act(async () => view().selection.enter(4))
    await click(3, true)
    expect(view().selection.positions).toEqual([3, 4])
  })
})
