import { captureSession, isSessionCurrent } from '@/stores/session-store'
import { create } from 'zustand'
import { api, onLastFmStatusChanged } from '@/lib/api'
import type { LastFmStatus } from '@/types'

interface LastFmState {
  status: LastFmStatus | null
  setStatus: (status: LastFmStatus) => void
  hydrate: () => Promise<void>
}

export const useLastFmStore = create<LastFmState>(set => ({
  status: null,
  setStatus: status => set({ status }),
  hydrate: async () => {
    const generation = captureSession()
    const status = await api.getLastFmStatus()
    if (isSessionCurrent(generation)) set({ status })
  },
}))

export async function startLastFmStatusListener() {
  const generation = captureSession()
  return onLastFmStatusChanged((status) => {
    if (isSessionCurrent(generation)) useLastFmStore.getState().setStatus(status)
  })
}
