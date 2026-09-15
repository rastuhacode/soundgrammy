import { usePlayerStore } from '@/stores/player-store'
import { useRepeatStore } from '@/stores/repeat-store'
import type { AudioEngine } from './engine'

export interface PlaybackActivity {
  notifyPlaying(): void
  notifyActivityStopped(): void
  notifyCompleted(restartSameTrack?: boolean): void
}
let playerLifetime = 0

/** Queue policy stays here, above transport. Subscribe synchronously to retain A → B → A. */
export function connectPlayerEngine(engine: AudioEngine, activity: PlaybackActivity) {
  const lifetime = ++playerLifetime
  let sequence = 0
  let attemptId: string | null = null
  let trackId: number | null = null
  let epoch = -1
  let actuallyPlaying = false
  let lastRevision = -1
  let completedAttempt: string | null = null
  let stopped = false

  const loadCurrent = () => {
    const player = usePlayerStore.getState()
    trackId = player.currentTrack?.id ?? null
    epoch = player.listenAttemptEpoch
    attemptId = trackId === null ? null : `${lifetime}:${++sequence}`
    completedAttempt = null
    if (!player.currentTrack || attemptId === null) {
      void engine.unload()
      return
    }
    void engine.load({
      trackId: player.currentTrack.id, attemptId,
      expectedDurationSeconds: player.currentTrack.duration,
    })
    if (player.isPlaying) void engine.play()
    else void engine.pause()
  }

  const unsubscribeEngine = engine.subscribe((event) => {
    if (stopped) return
    const snapshot = event.type === 'state' ? event.snapshot : event
    if (snapshot.attemptId !== attemptId || snapshot.trackId !== trackId) return
    const revision = snapshot.revision
    if (revision < lastRevision) return
    lastRevision = revision

    if (event.type === 'state') {
      const playing = event.snapshot.status === 'playing'
      if (actuallyPlaying !== playing) {
        actuallyPlaying = playing
        if (playing) activity.notifyPlaying()
        else activity.notifyActivityStopped()
      }
      if (event.snapshot.status === 'paused' || event.snapshot.status === 'error') {
        if (usePlayerStore.getState().isPlaying) usePlayerStore.getState().setPlaying(false)
      }
      else if (playing && !usePlayerStore.getState().isPlaying) {
        usePlayerStore.getState().setPlaying(true)
      }
      return
    }

    if (completedAttempt === event.attemptId) return
    completedAttempt = event.attemptId
    const player = usePlayerStore.getState()
    const repeat = useRepeatStore.getState().repeat
    const last = player.queue.cursor === player.queue.tracks.length - 1
    const nextId = last && repeat === 'none'
      ? null
      : player.queue.tracks[last ? 0 : player.queue.cursor + 1]?.id
    const restartSameTrack = repeat === 'one' || nextId === trackId
    activity.notifyCompleted(restartSameTrack)
    if (repeat !== 'one') player.playNext({
      reason: 'completed',
    })
    if (restartSameTrack) loadCurrent()
  })

  const unsubscribePlayer = usePlayerStore.subscribe((player, previous) => {
    if (stopped) return
    if ((player.currentTrack?.id ?? null) !== trackId || player.listenAttemptEpoch !== epoch) {
      loadCurrent()
    }
    else if (player.isPlaying !== previous.isPlaying) {
      if (player.isPlaying) {
        const snapshot = engine.getSnapshot()
        if (snapshot.status === 'ended' || completedAttempt === attemptId) {
          loadCurrent()
          // Preserve an explicit seek made after completion when opening the next attempt.
          if (snapshot.currentTimeSeconds < snapshot.durationSeconds) void engine.seek(snapshot.currentTimeSeconds)
        }
        else void engine.play()
      }
      else void engine.pause()
    }
  })
  loadCurrent()

  return () => {
    stopped = true
    unsubscribePlayer()
    unsubscribeEngine()
    activity.notifyActivityStopped()
    void engine.unload()
  }
}
