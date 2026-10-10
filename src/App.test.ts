// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import App from './App'
import { api } from '@/lib/api'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { deferred, playlists } from '@/test-support/fixtures'

const androidBack = vi.hoisted(() => ({ active: false, close: () => {} }))
vi.mock('@/hooks/use-app-session', () => ({
  useAppSession: () => ({ status: 'ready', session: {}, handleAuthenticated: vi.fn(), resetToLogin: vi.fn() }),
}))
vi.mock('@/hooks/use-compact-display', () => ({ useCompactDisplay: () => ({ isCompact: true }) }))
vi.mock('@/hooks/use-touch-screen', () => ({ useTouchScreen: () => true }))
vi.mock('@/hooks/use-android-back', async importOriginal => ({
  ...await importOriginal<typeof import('@/hooks/use-android-back')>(),
  useAndroidBackAction: (active: boolean, close: () => void) => Object.assign(androidBack, { active, close }),
}))
vi.mock('@/lib/api', () => ({ api: { deletePlaylist: vi.fn() } }))
vi.mock('@/lib/library-hydration', () => ({ hydrateLibrary: vi.fn() }))
vi.mock('@/components/auth/MtprotoLogin', () => ({ MtprotoLogin: () => null }))
vi.mock('@/components/audio/AudioPlayer', () => ({ AudioPlayer: () => null }))
vi.mock('@/components/playlist/PlaylistJobResults', () => ({ PlaylistJobResults: () => null }))
vi.mock('@/components/SidebarDrawer', () => ({ SidebarDrawer: () => null }))
vi.mock('@/components/playlist/SidebarPlaylistThumbnail', () => ({ SidebarPlaylistThumbnail: () => null }))
vi.mock('@/components/playlist/PlaylistFormDialog', () => ({ PlaylistFormDialog: () => null }))
vi.mock('@/components/playlist/PlaylistView', () => ({
  PlaylistView: ({ onBack }: { onBack: () => void }) =>
    createElement('button', { onClick: onBack }, 'Back'),
}))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

let host: HTMLDivElement
let root: ReturnType<typeof createRoot>

beforeEach(async () => {
  vi.clearAllMocks()
  localStorage.clear()
  usePlaylistsStore.getState().reset()
  usePlaylistsStore.getState().hydrate(playlists())
  vi.mocked(api.deletePlaylist).mockResolvedValue(undefined)
  host = document.createElement('div')
  document.body.append(host)
  root = createRoot(host)
  await act(async () => root.render(createElement(App)))
})

afterEach(async () => {
  await act(async () => root.unmount())
  host.remove()
})

async function click(label: string) {
  const button = Array.from(document.body.querySelectorAll('button, [role="button"], [role="menuitem"]'))
    .find(item => item.getAttribute('aria-label') === label || item.textContent === label)
  expect(button).toBeDefined()
  await act(async () => button!.dispatchEvent(new MouseEvent('click', { bubbles: true })))
}

it.each(['on-screen', 'Android'])('stays on the playlist list after %s Back and deletion of the last-viewed playlist', async (back) => {
  const deletion = deferred<void>()
  vi.mocked(api.deletePlaylist).mockReturnValue(deletion.promise)
  await click('Select Custom playlist')
  expect(host.querySelector('main')).not.toBeNull()
  if (back === 'Android') {
    expect(androidBack.active).toBe(true)
    await act(async () => androidBack.close())
  }
  else {
    await click('Back')
  }
  await click('Custom options')
  expect(host.querySelector('main')).toBeNull()
  await click('Delete playlist')
  expect(api.deletePlaylist).toHaveBeenCalledExactlyOnceWith(20)
  expect(host.querySelector('main')).toBeNull()
  await act(async () => deletion.resolve())

  expect(api.deletePlaylist).toHaveBeenCalledExactlyOnceWith(20)
  expect(usePlaylistsStore.getState().selectedPlaylistId).toBe('all')
  expect(usePlaylistsStore.getState().data?.custom).toEqual([])
  expect(host.querySelector('main')).toBeNull()
  expect(host.querySelector('aside')?.classList.contains('hidden')).toBe(false)
})

it.each(['Enter', ' '])('opens the playlist with %s on the row without selecting it from its options button', async (key) => {
  const options = host.querySelector('button[aria-label="Custom options"]')!
  await act(async () => options.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true })))
  expect(host.querySelector('main')).toBeNull()
  expect(usePlaylistsStore.getState().selectedPlaylistId).toBe('all')

  const row = host.querySelector('[aria-label="Select Custom playlist"]')!
  await act(async () => row.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true })))
  expect(host.querySelector('main')).not.toBeNull()
  expect(usePlaylistsStore.getState().selectedPlaylistId).toBe(20)
})

it('can reopen the same playlist and open a playlist selected elsewhere after Back', async () => {
  await click('Select Custom playlist')
  await click('Back')
  await click('Select Custom playlist')
  expect(host.querySelector('main')).not.toBeNull()
  await click('Back')
  await act(async () => usePlaylistsStore.getState().setSelectedPlaylist('liked'))
  expect(host.querySelector('main')).not.toBeNull()
})

it('keeps an already open track view visible when its playlist is deleted', async () => {
  await click('Select Custom playlist')
  await act(async () => usePlaylistsStore.getState().deletePlaylist(20))
  expect(usePlaylistsStore.getState().selectedPlaylistId).toBe('all')
  expect(host.querySelector('main')).not.toBeNull()
})
