// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { beforeEach, expect, it, vi } from 'vitest'
import { PlaylistView } from './PlaylistView'
import { useLibraryStore } from '@/stores/library-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { playlists, track } from '@/test-support/fixtures'

const layout = vi.hoisted(() => ({ isCompact: true }))
vi.mock('@/hooks/use-compact-display', () => ({ useCompactDisplay: () => layout }))
vi.mock('@/hooks/use-touch-screen', () => ({ useTouchScreen: () => layout.isCompact }))
vi.mock('./PlaylistTrackThumbnail', () => ({ TrackThumbnail: () => null }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    measure: vi.fn(), getTotalSize: () => count * 88,
    getVirtualItems: () => Array.from({ length: count }, (_, index) => ({ index, start: index * 88 })),
  }),
}))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

beforeEach(() => {
  vi.restoreAllMocks()
  layout.isCompact = true
  useLibraryStore.getState().setLibrary([track(1), track(2), track(3)])
  usePlaylistsStore.getState().hydrate(playlists())
  usePlaylistsStore.getState().setSelectedPlaylist('all')
})

async function click(element: Element | null) {
  expect(element).not.toBeNull()
  await act(async () => element!.dispatchEvent(new MouseEvent('click', { bubbles: true })))
}

it('opens compact search, filters results, and clears the query when closed', async () => {
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  try {
    await act(async () => root.render(createElement(PlaylistView)))
    await click(host.querySelector('button[aria-label="Search tracks"]'))
    const dialog = document.body.querySelector('[role="dialog"]')!
    const input = dialog.querySelector('input')!
    expect(dialog.querySelectorAll('[role="row"][aria-selected]')).toHaveLength(3)
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, 'Track 2')
      input.dispatchEvent(new Event('input', { bubbles: true }))
    })
    expect(dialog.querySelectorAll('[role="row"][aria-selected]')).toHaveLength(1)
    expect(dialog.textContent).toContain('Track 2')
    await click(dialog.querySelector('button[aria-label="Close track search"]'))
    expect(host.querySelectorAll('[role="row"][aria-selected]')).toHaveLength(3)
  }
  finally {
    await act(async () => root.unmount())
    host.remove()
  }
})

it.each([false, true])('offers creation with existing playlists=%s and adds selected tracks without import', async (hasPlaylists) => {
  if (!hasPlaylists) usePlaylistsStore.getState().hydrate({ ...playlists(), custom: [] })
  const create = vi.spyOn(usePlaylistsStore.getState(), 'createPlaylist').mockResolvedValue({
    id: 30, name: 'Selection', trackIds: [1, 3], updatedAt: '',
  })
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  try {
    await act(async () => root.render(createElement(PlaylistView)))
    await click(host.querySelector('button[aria-label="Track 1 options"]'))
    await click(Array.from(document.body.querySelectorAll('[role="menuitem"]')).find(el => el.textContent === 'Select')!)
    await click(host.querySelectorAll('[role="row"][aria-selected]')[2])
    await click(host.querySelector('button[aria-label="Actions for 2 selected tracks"]'))
    await click(document.body.querySelector('[data-slot="dropdown-menu-sub-trigger"]'))
    const menuItems = Array.from(document.body.querySelectorAll('[role="menuitem"]'))
    expect(menuItems.some(el => el.textContent === 'Custom')).toBe(hasPlaylists)
    const submenu = document.body.querySelector('[data-slot="dropdown-menu-sub-content"]')!
    expect(submenu.querySelector('[role="separator"]') !== null).toBe(hasPlaylists)
    await click(menuItems.find(el => el.textContent === 'Create playlist')!)
    const dialog = document.body.querySelector('[role="dialog"]')!
    expect(dialog.textContent).toContain('2 selected tracks')
    expect(dialog.querySelector('[role="tab"]')).toBeNull()
    const input = dialog.querySelector('input')!
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, 'Selection')
      input.dispatchEvent(new Event('input', { bubbles: true }))
    })
    await act(async () => dialog.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })))
    expect(create).toHaveBeenCalledExactlyOnceWith('Selection', [1, 3])
  }
  finally {
    await act(async () => root.unmount())
    host.remove()
  }
})

it.each([
  ['dropdown', false], ['dropdown', true], ['context', false], ['context', true],
] as const)('creates from a solo track in the %s menu with existing playlists=%s', async (variant, hasPlaylists) => {
  layout.isCompact = variant === 'dropdown'
  if (!hasPlaylists) usePlaylistsStore.getState().hydrate({ ...playlists(), custom: [] })
  const create = vi.spyOn(usePlaylistsStore.getState(), 'createPlaylist').mockResolvedValue({
    id: 30, name: 'Solo', trackIds: [2], updatedAt: '',
  })
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  try {
    await act(async () => root.render(createElement(PlaylistView)))
    if (variant === 'dropdown') {
      await click(host.querySelector('button[aria-label="Track 2 options"]'))
    }
    else {
      await act(async () => host.querySelectorAll('[role="row"][aria-selected]')[1]
        .dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 100, clientY: 100 })))
    }
    await click(document.body.querySelector(`[data-slot="${variant}-menu-sub-trigger"]`))
    const submenu = document.body.querySelector(`[data-slot="${variant}-menu-sub-content"]`)!
    expect(submenu.textContent).toContain('Create playlist')
    expect(submenu.querySelector('[role="separator"]') !== null).toBe(hasPlaylists)
    await click(Array.from(submenu.querySelectorAll('[role="menuitem"]')).find(el => el.textContent === 'Create playlist')!)
    const dialog = document.body.querySelector('[role="dialog"]')!
    expect(dialog.textContent).toContain('1 selected track')
    expect(dialog.querySelector('[role="tab"]')).toBeNull()
    const input = dialog.querySelector('input')!
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, 'Solo')
      input.dispatchEvent(new Event('input', { bubbles: true }))
    })
    await act(async () => dialog.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })))
    expect(create).toHaveBeenCalledExactlyOnceWith('Solo', [2])
  }
  finally {
    await act(async () => root.unmount())
    host.remove()
  }
})
