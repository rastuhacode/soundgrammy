export function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string' && message.length > 0) return message
  }
  if (error instanceof Error && error.message) return error.message
  return 'Something went wrong'
}

export function fieldErrors(errors: unknown[]): Array<{ message?: string }> {
  return errors.map((error) => {
    if (typeof error === 'string') return { message: error }
    if (error && typeof error === 'object' && 'message' in error) {
      const message = (error as { message?: unknown }).message
      return { message: typeof message === 'string' ? message : undefined }
    }
    return {}
  })
}
