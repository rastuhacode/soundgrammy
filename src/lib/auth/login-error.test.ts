import { describe, expect, it } from 'vitest'
import { loginErrorMessage } from './login-error'

describe('loginErrorMessage', () => {
  it('preserves Tauri string errors', () => {
    expect(loginErrorMessage(' incorrect password ', 'Invalid password')).toBe(
      'incorrect password',
    )
  })

  it('reads error-like objects', () => {
    expect(loginErrorMessage(
      { message: 'no password step in progress' },
      'Invalid password',
    )).toBe('no password step in progress')
  })

  it('uses the fallback for unknown errors', () => {
    expect(loginErrorMessage(null, 'Invalid password')).toBe('Invalid password')
  })
})
