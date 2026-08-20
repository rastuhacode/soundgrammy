import { describe, expect, it } from 'vitest'
import { resolvePlaybackShortcut } from './use-audio-controls'

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

  it('requires exact shortcuts and ignores key repeat', () => {
    expect(resolvePlaybackShortcut(keyboardEvent({
      code: 'ArrowRight',
    }))).toBeNull()
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
  })

  it('respects keyboard events handled by a component', () => {
    expect(resolvePlaybackShortcut(keyboardEvent({
      code: 'Space',
      defaultPrevented: true,
    }))).toBeNull()
  })
})
