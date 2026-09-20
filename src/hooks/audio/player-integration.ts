import { applyPlaybackSession, usePlayerStore } from '@/stores/player-store'
import type { AudioEngine } from './engine'

export interface PlaybackActivity {
  notifyPlaying(): void
  notifyActivityStopped(): void
  notifyCompleted(restartSameTrack?: boolean): void
}

/** Native attempt IDs identify queue memberships even when track IDs repeat. */
function isCurrentNativeAttempt(trackId: number | null, attemptId: string | null): boolean {
  const player = usePlayerStore.getState()
  return trackId === (player.currentTrack?.id ?? null)
    && attemptId === (player.currentTrack ? `native:${player.listenAttemptEpoch}` : null)
}

/** Subscribe to the native owner. No queue decisions or transport writes occur here. */
export function connectPlayerEngine(engine: AudioEngine, activity: PlaybackActivity) {
  let actuallyPlaying = false
  let attemptId: string | null = null
  let completedAttempt: string | null = null
  const accept = () => {
    const snapshot = engine.getSnapshot()
    if (snapshot.player) {
      applyPlaybackSession(snapshot.player)
      if (!isCurrentNativeAttempt(snapshot.trackId, snapshot.attemptId)) return
    }
    const playing = snapshot.status === 'playing'
    if (attemptId !== snapshot.attemptId) {
      // Store hydration above closes/opens the corresponding listen attempt.
      actuallyPlaying = false
      attemptId = snapshot.attemptId
    }
    if (playing !== actuallyPlaying) {
      actuallyPlaying = playing
      if (playing) activity.notifyPlaying()
      else activity.notifyActivityStopped()
    }
  }
  const unsubscribe = engine.subscribe((event) => {
    if (event.type === 'state') accept()
    else if (event.attemptId === attemptId && completedAttempt !== event.attemptId
      && (engine.kind !== 'native-rust' || isCurrentNativeAttempt(event.trackId, event.attemptId))) {
      completedAttempt = event.attemptId
      activity.notifyCompleted(false)
    }
  })
  accept()
  return () => {
    unsubscribe()
    activity.notifyActivityStopped()
  }
}
