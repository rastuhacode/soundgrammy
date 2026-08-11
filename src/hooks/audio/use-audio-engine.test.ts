import { describe, expect, it } from 'vitest'
import { isExpectedPlayInterruption } from './use-audio-engine'

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
