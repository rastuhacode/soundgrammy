// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { describe, expect, it, vi } from 'vitest'
import type { Track } from '@/lib/db'
import type { ResolvedSelectedPlaylist } from '@/stores/playlists-store'
import { PlaylistTrackDropdownMenu } from './PlaylistTrackDropdownMenu'

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

const track: Track = {
  id: 1,
  tg_user_id: 1,
  file_id: 'file',
  file_unique_id: 'unique',
  title: 'Test track',
  performer: 'Artist',
  duration: 100,
  source: 'telegram',
  mime_type: null,
  file_size: null,
  created_at: '2026-01-01',
}

const playlist: ResolvedSelectedPlaylist = {
  id: 'all',
  name: 'All tracks',
  trackIds: [1],
  isCustom: false,
  tracks: [track],
}

describe('touch track options', () => {
  it('opens the actions menu on tap without selecting a track', async () => {
    const host = document.createElement('div')
    document.body.append(host)
    const root = createRoot(host)
    const onSelect = vi.fn()

    try {
      await act(async () => root.render(createElement(PlaylistTrackDropdownMenu, {
        track,
        sourceIndex: 0,
        isLiked: false,
        currentPlaylist: playlist,
        customPlaylists: [],
        onSelect,
        onToggleLike: vi.fn(),
        onAddToPlaylist: vi.fn(),
        onDeleteFromPlaylist: vi.fn(),
        onPlayNext: vi.fn(),
        onAddToEnd: vi.fn(),
        onCache: vi.fn(),
        onDownload: vi.fn(),
        onRemoveFromCache: vi.fn(),
        onShowInfo: vi.fn(),
      })))

      const button = host.querySelector('button[aria-label="Test track options"]')!
      await act(async () => button.dispatchEvent(new MouseEvent('click', { bubbles: true })))

      expect(document.body.querySelector('[data-slot="dropdown-menu-content"]')).not.toBeNull()
      expect(document.body.textContent).toContain('Play next')
      expect(onSelect).not.toHaveBeenCalled()
    }
    finally {
      await act(async () => root.unmount())
      host.remove()
    }
  })
})
