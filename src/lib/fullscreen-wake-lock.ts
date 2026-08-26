import { api } from '@/lib/api'

type NativeWakeSetter = (enabled: boolean) => Promise<void>

/**
 * Serializes native wake updates so rapid fullscreen/setting transitions
 * cannot leave an older request as the final operating-system state.
 */
export function createWakeLockScheduler(setNative: NativeWakeSetter) {
  let queue = Promise.resolve()

  return (enabled: boolean): Promise<void> => {
    queue = queue
      .then(() => setNative(enabled), () => setNative(enabled))
      // Backend failures are already recorded by the shared API logger. Wake
      // inhibition is best-effort and must never block fullscreen playback.
      .catch(() => {})
    return queue
  }
}

export const updateFullscreenWakeLock = createWakeLockScheduler(
  enabled => api.setFullscreenDisplayAwake(enabled),
)
