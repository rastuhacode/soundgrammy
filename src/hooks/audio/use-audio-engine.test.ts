import { describe, expect, it } from 'vitest'
import {
  canSyncMediaPlaybackState,
  isExpectedPlayInterruption,
  registerMediaSessionTrackActions,
} from './use-audio-engine'

describe('isExpectedPlayInterruption', () => {
  it('recognizes pause/load AbortError as expected playback control flow', () => {
    expect(isExpectedPlayInterruption(new DOMException(
      'The play() request was interrupted by a call to pause().',
      'AbortError',
    ))).toBe(true)
  })

  it('does not hide real playback failures', () => {
    expect(isExpectedPlayInterruption(new DOMException(
      'Playback is not allowed.',
      'NotAllowedError',
    ))).toBe(false)
    expect(isExpectedPlayInterruption(new Error('decoder failed'))).toBe(false)
  })
})

describe('canSyncMediaPlaybackState', () => {
  const stablePlayback = {
    trackId: 42,
    loadedTrackId: 42,
    pendingSeek: null,
    isSeeking: false,
    sourceFailed: false,
  }

  it('allows native controls to sync a stable current source', () => {
    expect(canSyncMediaPlaybackState(stablePlayback)).toBe(true)
  })

  it('ignores playback events from source replacement', () => {
    expect(canSyncMediaPlaybackState({
      ...stablePlayback,
      loadedTrackId: null,
    })).toBe(false)
    expect(canSyncMediaPlaybackState({
      ...stablePlayback,
      loadedTrackId: 41,
    })).toBe(false)
  })

  it('ignores temporary seek pauses and failed sources', () => {
    expect(canSyncMediaPlaybackState({
      ...stablePlayback,
      pendingSeek: 120,
    })).toBe(false)
    expect(canSyncMediaPlaybackState({
      ...stablePlayback,
      isSeeking: true,
    })).toBe(false)
    expect(canSyncMediaPlaybackState({
      ...stablePlayback,
      sourceFailed: true,
    })).toBe(false)
  })
})

describe('registerMediaSessionTrackActions', () => {
  it('registers native next/previous handlers and removes them on cleanup', () => {
    const calls: Array<{
      action: 'nexttrack' | 'previoustrack'
      handler: MediaSessionActionHandler | null
    }> = []
    const nexttrack = () => {}
    const previoustrack = () => {}

    const cleanup = registerMediaSessionTrackActions({
      setActionHandler: (action, handler) => calls.push({ action, handler }),
    }, { nexttrack, previoustrack })

    expect(calls).toEqual([
      { action: 'nexttrack', handler: nexttrack },
      { action: 'previoustrack', handler: previoustrack },
    ])

    cleanup()
    expect(calls.slice(2)).toEqual([
      { action: 'nexttrack', handler: null },
      { action: 'previoustrack', handler: null },
    ])
  })

  it('keeps supported actions when a WebView rejects another one', () => {
    const registered: string[] = []

    const cleanup = registerMediaSessionTrackActions({
      setActionHandler: (action) => {
        if (action === 'previoustrack') throw new DOMException('Unsupported')
        registered.push(action)
      },
    }, {
      nexttrack: () => {},
      previoustrack: () => {},
    })

    expect(registered).toEqual(['nexttrack'])
    expect(cleanup).not.toThrow()
  })
})
