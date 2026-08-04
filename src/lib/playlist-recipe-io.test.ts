import { describe, expect, it } from 'vitest'
import { formatInvokeError } from './playlist-recipe-io'

describe('formatInvokeError', () => {
  it('returns trimmed strings as-is', () => {
    expect(formatInvokeError('missing field playlist_id')).toBe(
      'missing field playlist_id',
    )
  })

  it('reads message from plain objects', () => {
    expect(formatInvokeError({ message: 'Not authorized' })).toBe(
      'Not authorized',
    )
  })

  it('reads nested Tauri message payloads', () => {
    expect(
      formatInvokeError({ message: { message: 'Invalid playlist file' } }),
    ).toBe('Invalid playlist file')
  })

  it('uses Error.message when present', () => {
    expect(formatInvokeError(new Error('boom'))).toBe('boom')
  })

  it('falls back for unknown values', () => {
    expect(formatInvokeError(null)).toBe('Something went wrong')
    expect(formatInvokeError(undefined)).toBe('Something went wrong')
    expect(formatInvokeError('   ')).toBe('Something went wrong')
  })
})
