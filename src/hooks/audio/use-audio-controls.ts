import { useEffect, useRef } from 'react'
import {
  previousOrRestart,
  registerPlaybackController,
} from '@/lib/playback-controller'
import type { Track } from '@/lib/db'
import { usePlayerStore } from '@/stores/player-store'

// const MEDIA_SEEK_SECONDS = 10

const MediaSessionAction = {
  next_track: 'nexttrack',
  pause: 'pause',
  play: 'play',
  previous_track: 'previoustrack',
  // This events should be registered, but macOS spawns 15 seconds seek instead of next/previous buttons, which is less comfort
  // TODO: Check if it works fine on other platforms, then delete comments
  // seek_backward: 'seekbackward',
  // seek_forward: 'seekforward',
  seek_to: 'seekto',
  stop: 'stop',
} as const
type MediaSessionAction = typeof MediaSessionAction[keyof typeof MediaSessionAction]

export type PlaybackShortcut
  = 'toggle' | 'next' | 'previous' | 'seekForward' | 'seekBackward'

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
const INTERACTIVE_SELECTOR
  = `${EDITABLE_SELECTOR}, button, a[href], [role="button"], [role="checkbox"], [role="radio"], [role="switch"], [role="slider"], [role="menuitem"], [role="option"], [role="tab"]`
const KEYBOARD_SEEK_SECONDS = 5

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
    return targetMatchesClosest(event.target, INTERACTIVE_SELECTOR)
      ? null
      : 'toggle'
  }

  if (noModifiers) {
    if (targetMatchesClosest(event.target, INTERACTIVE_SELECTOR)) {
      return null
    }
    if (event.code === 'ArrowRight') return 'seekForward'
    if (event.code === 'ArrowLeft') return 'seekBackward'
    return null
  }

  const onePrimaryModifier = event.ctrlKey !== event.metaKey
  if (event.altKey || event.shiftKey || !onePrimaryModifier) return null

  if (event.code === 'ArrowRight') return 'next'
  if (event.code === 'ArrowLeft') return 'previous'
  return null
}

export function seekTargetByOffset(
  currentTime: number,
  duration: number,
  offset: number,
): number {
  const safeCurrentTime = Number.isFinite(currentTime) ? currentTime : 0
  if (!Number.isFinite(duration) || duration <= 0) {
    return Math.max(safeCurrentTime, 0)
  }
  return Math.min(Math.max(safeCurrentTime + offset, 0), duration)
}

interface SoundGrammyMediaSession {
  metadata: MediaMetadata | null
  playbackState: MediaSessionPlaybackState
  setPositionState: (state?: MediaPositionState) => void
  setActionHandler: (
    action: MediaSessionAction,
    handler: MediaSessionActionHandler | null,
  ) => void
}

export function mediaMetadataInit(
  track: Track,
  artworkUrl: string | null,
): MediaMetadataInit {
  return {
    title: track.title ?? 'Unknown Title',
    artist: track.performer ?? 'Unknown Artist',
    ...(artworkUrl
      ? { artwork: [{ src: artworkUrl }] }
      : {}),
  }
}

export function mediaPositionState(
  currentTime: number,
  duration: number,
): MediaPositionState | null {
  if (!Number.isFinite(duration) || duration <= 0) return null

  const position = Number.isFinite(currentTime)
    ? Math.min(Math.max(currentTime, 0), duration)
    : 0
  return { duration, playbackRate: 1, position }
}

/** Register each supported OS action independently for partial WebViews. */
export function registerMediaSessionActions(
  mediaSession: SoundGrammyMediaSession,
  handlers: Record<MediaSessionAction, MediaSessionActionHandler>,
): () => void {
  const registered: MediaSessionAction[] = []

  for (const action of Object.values(MediaSessionAction)) {
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
  track: Track | null
  artworkUrl: string | null
  isPlaying: boolean
  currentTime: number
  duration: number
  handleSeek: (time: number) => void
}

/** Connect app playback commands and OS media-session queue controls. */
export function useAudioControls({
  track,
  artworkUrl,
  isPlaying,
  currentTime,
  duration,
  handleSeek,
}: UseAudioControlsOptions) {
  const currentTimeRef = useRef(0)
  const durationRef = useRef(0)
  const handleSeekRef = useRef(handleSeek)

  useEffect(() => {
    currentTimeRef.current = currentTime
  }, [currentTime])

  useEffect(() => {
    durationRef.current = duration
  }, [duration])

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
      else if (shortcut === 'previous') {
        previousOrRestart()
      }
      else {
        const offset = shortcut === 'seekForward'
          ? KEYBOARD_SEEK_SECONDS
          : -KEYBOARD_SEEK_SECONDS
        const target = seekTargetByOffset(
          currentTimeRef.current,
          durationRef.current,
          offset,
        )
        if (target !== currentTimeRef.current) {
          currentTimeRef.current = target
          handleSeekRef.current(target)
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  useEffect(() => {
    if (typeof navigator === 'undefined' || !navigator.mediaSession) return

    return registerMediaSessionActions(navigator.mediaSession, {
      play: () => usePlayerStore.getState().setPlaying(true),
      pause: () => usePlayerStore.getState().setPlaying(false),
      stop: () => {
        usePlayerStore.getState().setPlaying(false)
        currentTimeRef.current = 0
        handleSeekRef.current(0)
      },
      seekto: (details) => {
        if (details.seekTime === undefined) return
        const target = seekTargetByOffset(
          details.seekTime,
          durationRef.current,
          0,
        )
        currentTimeRef.current = target
        handleSeekRef.current(target)
      },
      // seekforward: (details) => {
      //   const target = seekTargetByOffset(
      //     currentTimeRef.current,
      //     durationRef.current,
      //     details.seekOffset ?? MEDIA_SEEK_SECONDS,
      //   )
      //   currentTimeRef.current = target
      //   handleSeekRef.current(target)
      // },
      // seekbackward: (details) => {
      //   const target = seekTargetByOffset(
      //     currentTimeRef.current,
      //     durationRef.current,
      //     -(details.seekOffset ?? MEDIA_SEEK_SECONDS),
      //   )
      //   currentTimeRef.current = target
      //   handleSeekRef.current(target)
      // },
      nexttrack: () => usePlayerStore.getState().playNext(),
      previoustrack: previousOrRestart,
    })
  }, [])

  useEffect(() => {
    if (typeof navigator === 'undefined' || !navigator.mediaSession) return
    const mediaSession = navigator.mediaSession

    if (!track) {
      mediaSession.metadata = null
      return
    }
    if (typeof MediaMetadata === 'undefined') return

    const init = mediaMetadataInit(track, artworkUrl)
    try {
      mediaSession.metadata = new MediaMetadata(init)
    }
    catch {
      // A WebView may reject a custom/local artwork URL. Keep text metadata.
      try {
        mediaSession.metadata = new MediaMetadata(mediaMetadataInit(track, null))
      }
      catch {
        // Older WebViews can expose mediaSession without MediaMetadata support.
      }
    }
  }, [artworkUrl, track])

  useEffect(() => {
    if (typeof navigator === 'undefined' || !navigator.mediaSession) return
    navigator.mediaSession.playbackState = track
      ? (isPlaying ? 'playing' : 'paused')
      : 'none'
  }, [isPlaying, track])

  useEffect(() => {
    if (typeof navigator === 'undefined' || !navigator.mediaSession) return
    const position = track ? mediaPositionState(currentTime, duration) : null
    try {
      navigator.mediaSession.setPositionState(position ?? undefined)
    }
    catch {
      // Duration can change while a streamed MediaSource is being rebuilt.
    }
  }, [currentTime, duration, track])
}
