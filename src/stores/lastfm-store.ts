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
    const status = await api.getLastFmStatus()
    set({ status })
  },
}))

export async function startLastFmStatusListener() {
  return onLastFmStatusChanged(status => useLastFmStore.getState().setStatus(status))
}
