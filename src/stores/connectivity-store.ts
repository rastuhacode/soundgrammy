import { create } from 'zustand'

export type ConnectivityPhase = 'connecting' | 'online' | 'offline'

interface ConnectivityState {
  phase: ConnectivityPhase
  setConnecting: () => void
  setOnline: () => void
  setOffline: () => void
  reset: () => void
}

export const useConnectivityStore = create<ConnectivityState>(set => ({
  phase: 'connecting',
  setConnecting: () => set({ phase: 'connecting' }),
  setOnline: () => set({ phase: 'online' }),
  setOffline: () => set({ phase: 'offline' }),
  reset: () => set({ phase: 'connecting' }),
}))
