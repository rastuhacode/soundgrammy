import { beforeEach, describe, expect, it, vi } from 'vitest'

const { tauriInvoke, tauriListen, logError } = vi.hoisted(() => ({
  tauriInvoke: vi.fn(),
  tauriListen: vi.fn(),
  logError: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriInvoke,
  convertFileSrc: vi.fn((path: string) => path),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: tauriListen,
}))

vi.mock('@/lib/app-logger', () => ({
  appLogger: { error: logError },
}))

import { api, onLibraryBatch } from '@/lib/api'
import { libraryPage, track } from '@/test-support/fixtures'

describe('api diagnostics', () => {
  beforeEach(() => {
    tauriInvoke.mockReset()
    tauriListen.mockReset()
    logError.mockReset()
  })

  it('logs failed commands without including their arguments', async () => {
    const failure = new Error('sign in failed')
    tauriInvoke.mockRejectedValueOnce(failure)

    await expect(api.phoneCheckPassword('private-password')).rejects.toBe(failure)

    expect(logError).toHaveBeenCalledWith({
      source: 'backend',
      title: 'Backend command failed',
      description: 'The phone_check_password command returned an error.',
      error: failure,
      context: { command: 'phone_check_password' },
    })
    expect(JSON.stringify(logError.mock.calls)).not.toContain('private-password')
  })

  it('uses the fresh-session command when restarting QR login', async () => {
    tauriInvoke.mockResolvedValueOnce({
      status: 'waiting',
      url: 'tg://login?token=fresh',
      expires: 123,
    })

    await api.qrRestart()

    expect(tauriInvoke).toHaveBeenCalledWith('qr_restart', undefined)
  })

  it('rejects malformed library payloads with diagnostics that omit raw values', async () => {
    tauriInvoke.mockResolvedValueOnce([{ id: 'private-invalid-value' }])
    await expect(api.listTracks()).rejects.toThrow('list_tracks response')
    expect(logError).toHaveBeenCalledWith(expect.objectContaining({
      title: 'Invalid backend response', context: expect.objectContaining({ command: 'list_tracks' }),
    }))
    expect(JSON.stringify(logError.mock.calls)).not.toContain('private-invalid-value')
  })

  it('requests bounded display metadata pages with a snapshot revision', async () => {
    const page = libraryPage([track(501)], 500, null, 7)
    tauriInvoke.mockResolvedValueOnce(page)
    expect(await api.listTracksPage(500, 7)).toEqual(page)
    expect(tauriInvoke).toHaveBeenCalledWith('list_tracks_page', { offset: 500, limit: 500, revision: 7 })
  })

  it('uses soft sync by default and sends full mode only explicitly', async () => {
    tauriInvoke.mockResolvedValue({ changed: false, total: 0, lastSyncAt: null })
    await api.syncSavedMusic()
    await api.syncSavedMusic('full')
    expect(tauriInvoke).toHaveBeenNthCalledWith(1, 'sync_saved_music', { mode: 'soft' })
    expect(tauriInvoke).toHaveBeenNthCalledWith(2, 'sync_saved_music', { mode: 'full' })
  })

  it('rejects malformed page cursors without logging the raw payload', async () => {
    tauriInvoke.mockResolvedValueOnce({ ...libraryPage([track(1)]), nextOffset: 'private-invalid-cursor' })
    await expect(api.listTracksPage()).rejects.toThrow('list_tracks_page response')
    expect(JSON.stringify(logError.mock.calls)).not.toContain('private-invalid-cursor')
  })

  it('validates progressive events and drops malformed metadata without logging its values', async () => {
    const receive = vi.fn()
    tauriListen.mockResolvedValueOnce(vi.fn())
    await onLibraryBatch(receive)
    const listener = tauriListen.mock.calls[0]![1] as (event: { payload: unknown }) => void
    const page = libraryPage([track(1)])
    const payload = { tracks: page.tracks, status: page.status, removedTrackIds: [] }
    listener({ payload })
    expect(receive).toHaveBeenCalledExactlyOnceWith(payload)
    listener({ payload: { ...payload, tracks: [{ track: { id: 'private-invalid-event-value' }, position: 0 }] } })
    expect(receive).toHaveBeenCalledTimes(1)
    expect(logError).toHaveBeenCalledWith(expect.objectContaining({ title: 'Invalid backend event' }))
    expect(JSON.stringify(logError.mock.calls)).not.toContain('private-invalid-event-value')
  })
})
