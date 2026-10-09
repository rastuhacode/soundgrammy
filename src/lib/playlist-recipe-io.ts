import { save } from '@tauri-apps/plugin-dialog'
import { api } from '@/lib/api'
import type { PlaylistRecipeSource } from '@/types'

export { errorMessage as formatInvokeError } from '@/lib/errors'

function sanitizeExportBasename(name: string): string {
  const safe = name
    .trim()
    .replace(/[/\\:*?"<>|]/g, '_')
    .replace(/^\.+|\.+$/g, '')
  return safe || 'Playlist'
}

/** Opens a save dialog and writes a SoundGrammy playlist recipe JSON. */
export async function exportPlaylistRecipeFile(input: {
  source: PlaylistRecipeSource
  name: string
}): Promise<void> {
  const path = await save({
    defaultPath: `${sanitizeExportBasename(input.name)}.soundgrammy.json`,
    filters: [
      {
        name: 'SoundGrammy playlist',
        // Single-segment extensions only — multipart like "soundgrammy.json"
        // breaks the native save panel on some platforms.
        extensions: ['json'],
      },
    ],
  })
  if (!path) return
  await api.exportPlaylistJson(input.source, path)
}
