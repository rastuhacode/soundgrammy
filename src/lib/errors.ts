export function errorMessage(error: unknown): string {
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
