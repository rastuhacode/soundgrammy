import { beforeEach, describe, expect, it, vi } from 'vitest'

const { tauriInvoke, logError } = vi.hoisted(() => ({
  tauriInvoke: vi.fn(),
  logError: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriInvoke,
  convertFileSrc: vi.fn((path: string) => path),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

vi.mock('@/lib/app-logger', () => ({
  appLogger: { error: logError },
}))

import { api } from '@/lib/api'

describe('api diagnostics', () => {
  beforeEach(() => {
    tauriInvoke.mockReset()
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
})
