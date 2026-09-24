import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { api } from '@/lib/api'

export async function exportTrackAndReveal(trackId: number): Promise<void> {
  const location = await api.exportTrack(trackId)
  // Android returns a MediaStore URI, and the opener's reveal command is not
  // implemented on mobile. A reveal failure must not mark the export failed.
  if (location.startsWith('content://')) return
  try {
    await revealItemInDir(location)
  }
  catch {
    // The downloaded file is already saved; browsing to it is optional.
  }
}
