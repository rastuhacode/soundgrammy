import { useEffect, useRef } from 'react'
import {
  previousOrRestart,
} from '@/lib/playback-controller'
import { usePlayerStore } from '@/stores/player-store'

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

interface UseAudioControlsOptions {
  currentTime: number
  duration: number
  handleSeek: (time: number) => void
}

/** In-app shortcuts only. Rust owns OS media controls across WebView lifetimes. */
export function useAudioControls({
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
    const handleKeyDown = (event: KeyboardEvent) => {
      const shortcut = resolvePlaybackShortcut(event)
      if (!shortcut) return

      const player = usePlayerStore.getState()
      if (!player.currentTrack) return

      event.preventDefault()
      if (shortcut === 'toggle') {
        player.togglePlaying()
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
}
