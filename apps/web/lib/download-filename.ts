const MIME_EXTENSION: Record<string, string> = {
  "audio/mpeg": "mp3",
  "audio/mp3": "mp3",
  "audio/mp4": "m4a",
  "audio/m4a": "m4a",
  "audio/ogg": "ogg",
  "audio/opus": "opus",
  "audio/flac": "flac",
  "audio/wav": "wav",
  "audio/x-wav": "wav",
  "audio/aac": "aac",
};

const INVALID_FILENAME_CHARS = new Set("<>:\"/\\|?*");

/** Strips characters that are illegal or unsafe in download filenames. */
export function sanitizeDownloadFilename(name: string): string {
  let sanitized = "";
  for (const char of name) {
    const code = char.charCodeAt(0);
    sanitized += code < 32 || INVALID_FILENAME_CHARS.has(char) ? "_" : char;
  }
  return sanitized
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 180);
}

/** Builds a download filename, preferring the Telegram document attribute name. */
export function buildTrackDownloadFilename(params: {
  attributeFileName?: string | null;
  title?: string | null;
  fileName?: string | null;
  trackId: number;
  mimeType?: string | null;
}): string {
  const { attributeFileName, title, fileName, trackId, mimeType } = params;

  if (attributeFileName) {
    return sanitizeDownloadFilename(attributeFileName);
  }

  const base = sanitizeDownloadFilename(
    title?.trim() || fileName?.trim() || `track-${trackId}`,
  );
  const ext = mimeType ? MIME_EXTENSION[mimeType] ?? mimeType.split("/")[1] : "mp3";
  if (base.toLowerCase().endsWith(`.${ext}`)) {
    return base;
  }
  return `${base}.${ext}`;
}
