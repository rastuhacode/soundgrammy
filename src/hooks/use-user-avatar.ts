import { useEffect, useState } from 'react'
import { api, fileSrc } from '@/lib/api'
import {
  type ConnectivityPhase,
  useConnectivityStore,
} from '@/stores/connectivity-store'

interface LoadedAvatar {
  userId: number
  src: string | null
}

export function canLoadUserAvatar(
  userId: number | null,
  phase: ConnectivityPhase,
): userId is number {
  return userId !== null && phase === 'online'
}

export function useUserAvatar(userId: number | null): string | null {
  const phase = useConnectivityStore(state => state.phase)
  const [avatar, setAvatar] = useState<LoadedAvatar | null>(null)

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect -- Avatar state mirrors an async backend lookup controlled by session availability. */
    if (userId === null) {
      setAvatar(null)
      return
    }
    if (!canLoadUserAvatar(userId, phase)) return

    let cancelled = false
    api
      .getUserAvatar()
      .then((path) => {
        if (!cancelled) {
          setAvatar({ userId, src: path ? fileSrc(path) : null })
        }
      })
      .catch(() => {
        // Keep the previous avatar; the next online transition retries.
      })

    return () => {
      cancelled = true
    }
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [userId, phase])

  return avatar?.userId === userId ? avatar.src : null
}
