import { describe, expect, it } from 'vitest'
import {
  mediaMetadataInit,
  mediaPositionState,
  resolvePlaybackShortcut,
  seekTargetByOffset,
} from './use-audio-controls'
import type { Track } from '@/lib/db'

const track: Track = {
  id: 1,
  tg_user_id: 2,
  file_id: 'file',
  file_unique_id: 'unique',
  title: 'Alison',
  performer: 'Slowdive',
  duration: 231,
  source: 'saved_music',
  mime_type: 'audio/mpeg',
  file_size: 123,
  created_at: '2026-08-26T00:00:00Z',
}

function keyboardEvent(
  overrides: Partial<Parameters<typeof resolvePlaybackShortcut>[0]> = {},
): Parameters<typeof resolvePlaybackShortcut>[0] {
  return {
    altKey: false,
    code: '',
    ctrlKey: false,
    defaultPrevented: false,
    metaKey: false,
    repeat: false,
    shiftKey: false,
    target: null,
    ...overrides,
  }
}

function targetClosestTo(
  match: (selector: string) => boolean,
): EventTarget {
  return {
    closest: (selector: string) => match(selector) ? {} : null,
  } as unknown as EventTarget
}

describe('resolvePlaybackShortcut', () => {
  it('maps Space to play/pause', () => {
    expect(resolvePlaybackShortcut(keyboardEvent({ code: 'Space' })))
      .toBe('toggle')
  })

  it('maps Ctrl/Cmd + arrows to next and previous', () => {
    expect(resolvePlaybackShortcut(keyboardEvent({
      code: 'ArrowRight',
      ctrlKey: true,
    }))).toBe('next')
    expect(resolvePlaybackShortcut(keyboardEvent({
      code: 'ArrowLeft',
      metaKey: true,
    }))).toBe('previous')
  })

  it('maps unmodified arrows to five-second seeking', () => {
    expect(resolvePlaybackShortcut(keyboardEvent({ code: 'ArrowRight' })))
      .toBe('seekForward')
    expect(resolvePlaybackShortcut(keyboardEvent({ code: 'ArrowLeft' })))
      .toBe('seekBackward')
  })

  it('requires exact shortcuts and ignores key repeat', () => {
    expect(resolvePlaybackShortcut(keyboardEvent({
      code: 'ArrowRight',
      ctrlKey: true,
      shiftKey: true,
    }))).toBeNull()
    expect(resolvePlaybackShortcut(keyboardEvent({
      code: 'Space',
      repeat: true,
    }))).toBeNull()
  })

  it('leaves focused editable and interactive controls alone', () => {
    expect(resolvePlaybackShortcut(keyboardEvent({
      code: 'ArrowRight',
      ctrlKey: true,
      target: targetClosestTo(selector => selector.startsWith('input')),
    }))).toBeNull()
    expect(resolvePlaybackShortcut(keyboardEvent({
      code: 'Space',
      target: targetClosestTo(selector => selector.includes('button')),
    }))).toBeNull()
    expect(resolvePlaybackShortcut(keyboardEvent({
      code: 'ArrowRight',
      target: targetClosestTo(selector => selector.includes('[role="slider"]')),
    }))).toBeNull()
  })

  it('respects keyboard events handled by a component', () => {
    expect(resolvePlaybackShortcut(keyboardEvent({
      code: 'Space',
      defaultPrevented: true,
    }))).toBeNull()
  })
})

describe('seekTargetByOffset', () => {
  it('moves by the requested offset', () => {
    expect(seekTargetByOffset(30, 100, 5)).toBe(35)
    expect(seekTargetByOffset(30, 100, -5)).toBe(25)
  })

  it('clamps seeking to the track bounds', () => {
    expect(seekTargetByOffset(2, 100, -5)).toBe(0)
    expect(seekTargetByOffset(98, 100, 5)).toBe(100)
    expect(seekTargetByOffset(10, 0, 5)).toBe(10)
  })
})

describe('mediaMetadataInit', () => {
  it('publishes track text and cached artwork', () => {
    expect(mediaMetadataInit(track, 'asset://localhost/cover.jpg')).toEqual({
      title: 'Alison',
      artist: 'Slowdive',
      artwork: [{
        src: 'asset://localhost/cover.jpg',
      }],
    })
  })

  it('provides readable fallbacks without inventing album data', () => {
    expect(mediaMetadataInit({
      ...track,
      title: null,
      performer: null,
    }, null)).toEqual({
      title: 'Unknown Title',
      artist: 'Unknown Artist',
    })
  })
})

describe('mediaPositionState', () => {
  it('publishes and clamps valid playback position', () => {
    expect(mediaPositionState(42, 231)).toEqual({
      duration: 231,
      playbackRate: 1,
      position: 42,
    })
    expect(mediaPositionState(250, 231)?.position).toBe(231)
    expect(mediaPositionState(-1, 231)?.position).toBe(0)
  })

  it('clears position state while duration is unavailable', () => {
    expect(mediaPositionState(0, 0)).toBeNull()
    expect(mediaPositionState(0, Number.NaN)).toBeNull()
  })
})
