import { usePlayerStore } from '@/stores/player-store'

const PREVIOUS_TRACK_THRESHOLD = 5 // 5s

interface PlaybackController {
  getCurrentTime: () => number
  seekTo: (time: number) => void
}

let controller: PlaybackController | null = null

export function registerPlaybackController(next: PlaybackController) {
  controller = next
  return () => {
    if (controller === next) controller = null
  }
}

/** Restart current track if past threshold; otherwise go to previous track. */
export function previousOrRestart() {
  if (!controller) {
    usePlayerStore.getState().playPrevious()
    return
  }

  if (controller.getCurrentTime() < PREVIOUS_TRACK_THRESHOLD) {
    usePlayerStore.getState().playPrevious()
  }
  else {
    controller.seekTo(0)
  }
}
