const MAX_PLAYLIST_THUMBNAIL_BYTES = 512 * 1024

const ALLOWED_THUMBNAIL_TYPES = new Set([
  'image/jpeg',
  'image/png',
  'image/webp',
])

export async function readPlaylistThumbnailFile(
  file: File,
): Promise<{ data: string, mime: 'image/jpeg' | 'image/png' | 'image/webp' }> {
  if (!ALLOWED_THUMBNAIL_TYPES.has(file.type)) {
    throw new Error('Use a JPEG, PNG, or WebP image')
  }

  if (file.size > MAX_PLAYLIST_THUMBNAIL_BYTES) {
    throw new Error('Image must be smaller than 512KB')
  }

  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(new Error('Failed to read image'))
    reader.readAsDataURL(file)
  })

  const [, base64 = ''] = dataUrl.split(',')
  if (!base64) {
    throw new Error('Failed to read image')
  }

  return {
    data: base64,
    mime: file.type as 'image/jpeg' | 'image/png' | 'image/webp',
  }
}

export function createThumbnailPreviewUrl(file: File): string {
  return URL.createObjectURL(file)
}

export function revokeThumbnailPreviewUrl(url: string | null) {
  if (url?.startsWith('blob:')) {
    URL.revokeObjectURL(url)
  }
}
