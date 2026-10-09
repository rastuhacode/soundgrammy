import { useCallback, useLayoutEffect, useRef } from 'react'
import { captureSession, isSessionCurrent } from '@/stores/session-store'

/** UI completions belong to both an account and one mounted/open view lifetime. */
export function useAsyncScope(enabled = true, identity: unknown = null) {
  const scope = useRef<{ active: boolean } | null>(null)
  useLayoutEffect(() => {
    const lifetime = { active: enabled }
    scope.current = lifetime
    return () => {
      lifetime.active = false
    }
  }, [enabled, identity])

  const capture = useCallback(() => {
    const lifetime = scope.current
    const generation = captureSession()
    return () => lifetime?.active === true && isSessionCurrent(generation)
  }, [])
  const invalidate = useCallback(() => {
    if (scope.current) scope.current.active = false
  }, [])
  return { capture, invalidate }
}
