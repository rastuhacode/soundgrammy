// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Track } from '@/lib/db'
import { PlaylistTrackRowView } from './PlaylistTrackRow'

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

vi.mock('./PlaylistTrackThumbnail', () => ({ TrackThumbnail: () => null }))

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

function pointerEvent(type: string, x = 0, y = 0) {
  const event = new MouseEvent(type, { bubbles: true, clientX: x, clientY: y })
  Object.defineProperty(event, 'pointerType', { value: 'touch' })
  return event
}

describe('touch track selection', () => {
  let host: HTMLDivElement
  let root: Root
  const onPlay = vi.fn()
  const onEnterSelection = vi.fn()
  const onOpenOptions = vi.fn()

  beforeEach(async () => {
    vi.useFakeTimers()
    onPlay.mockClear()
    onEnterSelection.mockClear()
    onOpenOptions.mockClear()
    host = document.createElement('div')
    document.body.append(host)
    root = createRoot(host)
    await act(async () => root.render(createElement(PlaylistTrackRowView, {
      track,
      isActive: false,
      isPlaying: false,
      isSelected: false,
      selectionMode: false,
      touchScreen: true,
      onRowClick: onPlay,
      onEnterSelection,
      touchOptions: createElement('button', {
        'aria-label': 'Test track options',
        'onPointerDown': (event: React.PointerEvent) => event.stopPropagation(),
        'onClick': (event: React.MouseEvent) => {
          event.stopPropagation()
          onOpenOptions()
        },
      }, '…'),
    })))
  })

  afterEach(async () => {
    await act(async () => root.unmount())
    host.remove()
    vi.useRealTimers()
  })

  it('selects on long press without playing the track', async () => {
    const row = host.querySelector('[role="row"]')!
    await act(async () => row.dispatchEvent(pointerEvent('pointerdown')))
    await act(async () => vi.advanceTimersByTime(500))
    await act(async () => row.dispatchEvent(pointerEvent('pointerup')))
    await act(async () => row.dispatchEvent(new MouseEvent('click', { bubbles: true })))

    expect(onEnterSelection).toHaveBeenCalledTimes(1)
    expect(onPlay).not.toHaveBeenCalled()
  })

  it('keeps short taps for playback and cancels a moving touch', async () => {
    const row = host.querySelector('[role="row"]')!
    await act(async () => row.dispatchEvent(pointerEvent('pointerdown')))
    await act(async () => row.dispatchEvent(pointerEvent('pointerup')))
    await act(async () => row.dispatchEvent(new MouseEvent('click', { bubbles: true })))
    expect(onPlay).toHaveBeenCalledTimes(1)

    await act(async () => row.dispatchEvent(pointerEvent('pointerdown')))
    await act(async () => row.dispatchEvent(pointerEvent('pointermove', 20)))
    await act(async () => vi.advanceTimersByTime(500))
    expect(onEnterSelection).not.toHaveBeenCalled()
  })

  it('opens track options on a short tap without selecting or playing', async () => {
    const button = host.querySelector('button[aria-label="Test track options"]')!
    await act(async () => button.dispatchEvent(pointerEvent('pointerdown')))
    await act(async () => button.dispatchEvent(new MouseEvent('click', { bubbles: true })))

    expect(onOpenOptions).toHaveBeenCalledTimes(1)
    expect(onEnterSelection).not.toHaveBeenCalled()
    expect(onPlay).not.toHaveBeenCalled()
  })

  it('shows separate drag and options controls for a custom playlist', async () => {
    const onTouchDragStart = vi.fn()
    await act(async () => root.render(createElement(PlaylistTrackRowView, {
      track,
      isActive: false,
      isPlaying: false,
      isSelected: false,
      selectionMode: false,
      touchScreen: true,
      canReorder: true,
      onRowClick: onPlay,
      onEnterSelection,
      onTouchDragStart,
      touchOptions: createElement('button', { 'aria-label': 'Test track options' }, '…'),
    })))

    const dragHandle = host.querySelector('button[aria-label="Drag Test track to reorder"]')!
    await act(async () => dragHandle.dispatchEvent(pointerEvent('pointerdown')))
    await act(async () => dragHandle.dispatchEvent(new Event('touchstart', { bubbles: true })))
    await act(async () => vi.advanceTimersByTime(500))

    expect(onTouchDragStart).toHaveBeenCalledTimes(1)
    expect(onEnterSelection).not.toHaveBeenCalled()
    expect(host.querySelector('button[aria-label="Test track options"]')).not.toBeNull()
  })
})
