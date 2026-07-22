import { describe, expect, it } from 'vitest'
import { buildProxyLink } from './proxy-link'

describe('buildProxyLink', () => {
  it('builds a tg://proxy link from fields', () => {
    expect(
      buildProxyLink('127.0.0.1', 1443, 'ddebb7baf7cdd71571bf6ea7a9daf32d29'),
    ).toBe(
      'tg://proxy?server=127.0.0.1&port=1443&secret=ddebb7baf7cdd71571bf6ea7a9daf32d29',
    )
  })

  it('trims whitespace', () => {
    expect(
      buildProxyLink('  127.0.0.1  ', 1443, '  ddebb7baf7cdd71571bf6ea7a9daf32d29  '),
    ).toContain('server=127.0.0.1')
  })

  it('returns empty when any required field is missing', () => {
    expect(buildProxyLink('', 1443, 'ddab')).toBe('')
    expect(buildProxyLink('127.0.0.1', 0, 'ddab')).toBe('')
    expect(buildProxyLink('127.0.0.1', 1443, '')).toBe('')
  })
})
