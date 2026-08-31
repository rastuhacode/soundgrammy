import { describe, expect, it } from 'vitest'
import { canLoadUserAvatar } from './use-user-avatar'

describe('canLoadUserAvatar', () => {
  it('waits for both a session and an online Telegram connection', () => {
    expect(canLoadUserAvatar(null, 'connecting')).toBe(false)
    expect(canLoadUserAvatar(42, 'connecting')).toBe(false)
    expect(canLoadUserAvatar(42, 'offline')).toBe(false)
    expect(canLoadUserAvatar(null, 'online')).toBe(false)
    expect(canLoadUserAvatar(42, 'online')).toBe(true)
  })
})
