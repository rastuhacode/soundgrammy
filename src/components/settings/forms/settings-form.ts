export { errorMessage } from '@/lib/errors'

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
