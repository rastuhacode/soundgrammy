import { save } from '@tauri-apps/plugin-dialog'
import { api } from '@/lib/api'
import type { PlaylistRecipeSource } from '@/types'

export function formatInvokeError(error: unknown): string {
  if (typeof error === 'string' && error.trim()) return error
  if (error && typeof error === 'object') {
    const record = error as Record<string, unknown>
    if (typeof record.message === 'string' && record.message.length > 0) {
      return record.message
    }
    // Tauri sometimes nests the payload.
    if (record.message && typeof record.message === 'object') {
      const nested = record.message as Record<string, unknown>
      if (typeof nested.message === 'string' && nested.message.length > 0) {
        return nested.message
      }
    }
    try {
      return JSON.stringify(error)
    }
    catch {
      // fall through
    }
  }
  if (error instanceof Error && error.message) return error.message
  return 'Something went wrong'
}

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
