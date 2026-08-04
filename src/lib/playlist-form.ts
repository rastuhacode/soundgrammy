/** Shared playlist create/import form helpers (kept free of React for unit tests). */

export function validatePlaylistName(value: string): string | null {
  const trimmed = value.trim()
  if (trimmed.length === 0) return 'Playlist name is required'
  if (trimmed.length > 100) return 'Playlist name must be at most 100 characters'
  return null
}

/** Last path segment for display (POSIX or Windows separators). */
export function fileBasename(path: string): string {
  const parts = path.split(/[/\\]/)
  return parts[parts.length - 1] || path
}
