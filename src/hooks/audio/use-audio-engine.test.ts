import { describe, expect, it } from 'vitest'
import {
  canSyncMediaPlaybackState,
  isExpectedPlayInterruption,
  registerMediaSessionActions,
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

describe('registerMediaSessionActions', () => {
  const handlers = {
    play: () => {},
    pause: () => {},
    stop: () => {},
    seekto: () => {},
    seekforward: () => {},
    seekbackward: () => {},
    nexttrack: () => {},
    previoustrack: () => {},
  }

  it('registers native playback handlers and removes them on cleanup', () => {
    const calls: Array<{
      action: keyof typeof handlers
      handler: MediaSessionActionHandler | null
    }> = []

    const cleanup = registerMediaSessionActions({
      metadata: null,
      playbackState: 'none',
      setPositionState: () => {},
      setActionHandler: (action, handler) => calls.push({ action, handler }),
    }, handlers)

    expect(calls.map(call => call.action)).toEqual(Object.keys(handlers))

    cleanup()
    expect(calls.slice(8)).toEqual(
      Object.keys(handlers).map(action => ({ action, handler: null })),
    )
  })

  it('keeps supported actions when a WebView rejects another one', () => {
    const registered: string[] = []

    const cleanup = registerMediaSessionActions({
      metadata: null,
      playbackState: 'none',
      setPositionState: () => {},
      setActionHandler: (action) => {
        if (action === 'previoustrack') throw new DOMException('Unsupported')
        registered.push(action)
      },
    }, handlers)

    expect(registered).toEqual(Object.keys(handlers).filter(
      action => action !== 'previoustrack',
    ))
    expect(cleanup).not.toThrow()
  })
})
