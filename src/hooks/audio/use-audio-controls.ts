import { useEffect, useRef } from 'react'
import {
  previousOrRestart,
  registerPlaybackController,
} from '@/lib/playback-controller'
import { usePlayerStore } from '@/stores/player-store'

type TrackMediaSessionAction = 'nexttrack' | 'previoustrack'

interface TrackMediaSession {
  setActionHandler: (
    action: TrackMediaSessionAction,
    handler: MediaSessionActionHandler | null,
  ) => void
}

/** Register only the queue actions a headset cannot express through <audio>. */
export function registerMediaSessionTrackActions(
  mediaSession: TrackMediaSession,
  handlers: Record<TrackMediaSessionAction, MediaSessionActionHandler>,
): () => void {
  const registered: TrackMediaSessionAction[] = []

  for (const action of ['nexttrack', 'previoustrack'] as const) {
    try {
      mediaSession.setActionHandler(action, handlers[action])
      registered.push(action)
    }
    catch {
      // WebViews may expose Media Session while omitting individual actions.
    }
  }

  return () => {
    for (const action of registered) {
      try {
        mediaSession.setActionHandler(action, null)
      }
      catch {
        // The platform can withdraw action support during teardown.
      }
    }
  }
}

interface UseAudioControlsOptions {
  currentTime: number
  handleSeek: (time: number) => void
}

/** Connect app playback commands and OS media-session queue controls. */
export function useAudioControls({
  currentTime,
  handleSeek,
}: UseAudioControlsOptions) {
  const currentTimeRef = useRef(0)
  const handleSeekRef = useRef(handleSeek)

  useEffect(() => {
    currentTimeRef.current = currentTime
  }, [currentTime])

  useEffect(() => {
    handleSeekRef.current = handleSeek
  }, [handleSeek])

  useEffect(() => {
    return registerPlaybackController({
      getCurrentTime: () => currentTimeRef.current,
      seekTo: (time) => {
        handleSeekRef.current(time)
      },
    })
  }, [])

  useEffect(() => {
    if (typeof navigator === 'undefined' || !navigator.mediaSession) return

    return registerMediaSessionTrackActions(navigator.mediaSession, {
      nexttrack: () => usePlayerStore.getState().playNext(),
      previoustrack: previousOrRestart,
    })
  }, [])
}
