import { usePlayerStore } from '@/stores/player-store'

/** The native owner uses its current position for restart/previous policy. */
export function previousOrRestart() {
  usePlayerStore.getState().previousOrRestart()
}
