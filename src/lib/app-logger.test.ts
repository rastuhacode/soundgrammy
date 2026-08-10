import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { appLogger, formatLogEntries } from '@/lib/app-logger'
import { MAX_LOG_ENTRIES, useLogStore } from '@/stores/log-store'

describe('appLogger', () => {
  beforeEach(() => {
    useLogStore.getState().clear()
    useLogStore.getState().setEnabled(false)
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.spyOn(console, 'info').mockImplementation(() => {})
  })

  afterEach(() => {
    useLogStore.getState().clear()
    useLogStore.getState().setEnabled(false)
    vi.restoreAllMocks()
  })

  it('does not retain entries while diagnostics are disabled', () => {
    appLogger.error({
      source: 'audio',
      title: 'Playback failed',
      error: new Error('decoder failed'),
    })

    expect(useLogStore.getState().entries).toEqual([])
    expect(console.error).toHaveBeenCalledOnce()
  })

  it('normalizes errors and redacts sensitive context', () => {
    useLogStore.getState().setEnabled(true)

    appLogger.error({
      source: 'backend',
      title: 'Command failed',
      error: new Error('network unavailable'),
      context: {
        command: 'refresh_auth',
        password: 'do-not-store-this',
        nested: { proxySecret: 'also-secret' },
      },
    })

    const [entry] = useLogStore.getState().entries
    expect(entry).toMatchObject({
      level: 'error',
      source: 'backend',
      title: 'Command failed',
      description: 'network unavailable',
    })
    expect(entry?.details).toContain('refresh_auth')
    expect(entry?.details).toContain('[REDACTED]')
    expect(entry?.details).not.toContain('do-not-store-this')
    expect(entry?.details).not.toContain('also-secret')
  })

  it('keeps only the newest bounded set of entries', () => {
    useLogStore.getState().setEnabled(true)

    for (let index = 0; index <= MAX_LOG_ENTRIES; index += 1) {
      appLogger.info({
        source: 'application',
        title: `Entry ${index}`,
      })
    }

    const entries = useLogStore.getState().entries
    expect(entries).toHaveLength(MAX_LOG_ENTRIES)
    expect(entries[0]?.title).toBe('Entry 1')
    expect(entries.at(-1)?.title).toBe(`Entry ${MAX_LOG_ENTRIES}`)
  })

  it('formats entries for copying in chronological order', () => {
    useLogStore.getState().setEnabled(true)
    appLogger.warning({ source: 'cache', title: 'First warning' })
    appLogger.error({ source: 'audio', title: 'Second error' })

    const output = formatLogEntries(useLogStore.getState().entries)
    expect(output.indexOf('First warning')).toBeLessThan(output.indexOf('Second error'))
    expect(output).toContain('WARNING cache')
    expect(output).toContain('ERROR audio')
  })
})
