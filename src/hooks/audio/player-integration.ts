import { applyPlaybackSession } from '@/stores/player-store'
import { captureSession, isSessionCurrent } from '@/stores/session-store'
import type { AudioEngine } from './engine'

/** Rust owns attempts, activity and queue decisions; the UI only mirrors snapshots. */
export function connectPlayerEngine(engine: AudioEngine) {
  const generation = captureSession()
  const accept = () => {
    if (!isSessionCurrent(generation)) return
    const session = engine.getSnapshot().player
    if (session) applyPlaybackSession(session)
  }
  const unsubscribe = engine.subscribe((event) => {
    if (event.type === 'state') accept()
  })
  accept()
  return unsubscribe
}
