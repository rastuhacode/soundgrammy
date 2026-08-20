import { useEffect, useRef } from 'react'
import {
  previousOrRestart,
  registerPlaybackController,
} from '@/lib/playback-controller'
import { usePlayerStore } from '@/stores/player-store'

type TrackMediaSessionAction = 'nexttrack' | 'previoustrack'

export type PlaybackShortcut = 'toggle' | 'next' | 'previous'

type PlaybackShortcutEvent = Pick<
  KeyboardEvent,
  | 'altKey'
  | 'code'
  | 'ctrlKey'
  | 'defaultPrevented'
  | 'metaKey'
  | 'repeat'
  | 'shiftKey'
  | 'target'
>

const EDITABLE_SELECTOR
  = 'input, textarea, select, [contenteditable]:not([contenteditable="false"])'
const SPACE_INTERACTIVE_SELECTOR
  = `${EDITABLE_SELECTOR}, button, a[href], [role="button"], [role="checkbox"], [role="radio"], [role="switch"], [role="slider"], [role="menuitem"], [role="option"], [role="tab"]`

function targetMatchesClosest(
  target: EventTarget | null,
  selector: string,
): boolean {
  if (!target || typeof (target as Element).closest !== 'function') return false
  return (target as Element).closest(selector) !== null
}

/** Resolve exact, global playback shortcuts without hijacking focused controls. */
export function resolvePlaybackShortcut(
  event: PlaybackShortcutEvent,
): PlaybackShortcut | null {
  if (
    event.defaultPrevented
    || event.repeat
    || targetMatchesClosest(event.target, EDITABLE_SELECTOR)
  ) {
    return null
  }

  const noModifiers
    = !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey
  if (event.code === 'Space' && noModifiers) {
    return targetMatchesClosest(event.target, SPACE_INTERACTIVE_SELECTOR)
      ? null
      : 'toggle'
  }

  const onePrimaryModifier = event.ctrlKey !== event.metaKey
  if (event.altKey || event.shiftKey || !onePrimaryModifier) return null

  if (event.code === 'ArrowRight') return 'next'
  if (event.code === 'ArrowLeft') return 'previous'
  return null
}

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
    const handleKeyDown = (event: KeyboardEvent) => {
      const shortcut = resolvePlaybackShortcut(event)
      if (!shortcut) return

      const player = usePlayerStore.getState()
      if (!player.currentTrack) return

      event.preventDefault()
      if (shortcut === 'toggle') {
        player.setPlaying(!player.isPlaying)
      }
      else if (shortcut === 'next') {
        player.playNext()
      }
      else {
        previousOrRestart()
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  useEffect(() => {
    if (typeof navigator === 'undefined' || !navigator.mediaSession) return

    return registerMediaSessionTrackActions(navigator.mediaSession, {
      nexttrack: () => usePlayerStore.getState().playNext(),
      previoustrack: previousOrRestart,
    })
  }, [])
}
