import { assertSession, captureSession, createSessionQueue, isSessionCurrent } from '@/stores/session-store'
import { create } from 'zustand'
import { api, onLastFmStatusChanged } from '@/lib/api'
import type { LastFmPendingAction, LastFmStatus } from '@/types'

interface LastFmState {
  status: LastFmStatus | null
  setStatus: (status: LastFmStatus) => void
  hydrate: () => Promise<void>
  revision: number
  saveEnabled: (enabled: boolean) => Promise<LastFmStatus>
  startAuth: () => Promise<LastFmStatus>
  completeAuth: () => Promise<LastFmStatus>
  cancelAuth: () => Promise<LastFmStatus>
  disconnect: (pendingAction?: LastFmPendingAction) => Promise<LastFmStatus>
}

const enqueue = createSessionQueue()
function mutate(action: () => Promise<LastFmStatus>) {
  return enqueue(async (generation) => {
    const status = await action()
    assertSession(generation)
    useLastFmStore.getState().setStatus(status)
    return status
  })
}

export const useLastFmStore = create<LastFmState>((set, get) => ({
  status: null,
  revision: 0,
  setStatus: status => set(state => ({ status, revision: state.revision + 1 })),
  saveEnabled: enabled => mutate(() => api.setLastFmEnabled(enabled)),
  startAuth: () => mutate(() => api.startLastFmAuth()),
  completeAuth: () => mutate(() => api.completeLastFmAuth()),
  cancelAuth: () => mutate(() => api.cancelLastFmAuth()),
  disconnect: pendingAction => mutate(() => api.disconnectLastFm(pendingAction)),
  hydrate: async () => {
    const generation = captureSession()
    const revision = get().revision
    const status = await api.getLastFmStatus()
    if (isSessionCurrent(generation) && get().revision === revision) get().setStatus(status)
  },
}))

export async function startLastFmStatusListener() {
  const generation = captureSession()
  return onLastFmStatusChanged((status) => {
    if (isSessionCurrent(generation)) useLastFmStore.getState().setStatus(status)
  })
}
