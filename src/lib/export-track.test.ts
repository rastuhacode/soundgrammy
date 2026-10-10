import { beforeEach, describe, expect, it, vi } from 'vitest'

const { exportTrack, revealItemInDir } = vi.hoisted(() => ({
  exportTrack: vi.fn(),
  revealItemInDir: vi.fn(),
}))

vi.mock('@/lib/api', () => ({ api: { exportTrack } }))
vi.mock('@tauri-apps/plugin-opener', () => ({ revealItemInDir }))

import { exportTrackAndReveal } from './export-track'

describe('exportTrackAndReveal', () => {
  beforeEach(() => vi.resetAllMocks())

  it('does not report an opener failure as a failed download', async () => {
    exportTrack.mockResolvedValue('/Downloads/SoundGrammy/song.mp3')
    revealItemInDir.mockRejectedValue('unsupported platform')

    await expect(exportTrackAndReveal(7)).resolves.toBeUndefined()
    expect(exportTrack).toHaveBeenCalledWith(7)
  })

  it('does not ask the opener to reveal Android MediaStore URIs', async () => {
    exportTrack.mockResolvedValue('content://media/external/downloads/7')

    await exportTrackAndReveal(7)
    expect(revealItemInDir).not.toHaveBeenCalled()
  })

  it('still reports a failed export', async () => {
    const error = { message: 'Cannot save track' }
    exportTrack.mockRejectedValue(error)

    await expect(exportTrackAndReveal(7)).rejects.toBe(error)
    expect(revealItemInDir).not.toHaveBeenCalled()
  })
})
