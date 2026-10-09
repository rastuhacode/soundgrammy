import { api, onNativeAudioEvent, onNativeAudioState } from '@/lib/api'
import { assertSession, createSessionQueue } from '@/stores/session-store'
import { readLegacyRepeat } from '@/stores/repeat-store'
import { readLegacyShuffle } from '@/stores/shuffle-store'
import type { PlayerCommandPort } from '@/types/playback'
import type { AudioTransportPort } from '@/types/audio'

// Player and transport requests use one native IPC order within an account.
const enqueue = createSessionQueue()
function command(operation: () => Promise<unknown>) {
  return enqueue(async (generation) => {
    const response = await operation()
    assertSession(generation)
    return response
  })
}

export const nativePlayerCommands: PlayerCommandPort = {
  command: value => command(() => api.nativePlayerCommand(value)),
  snapshot: () => command(() => api.nativeAudioSnapshot()),
}

export function attachPlayer(): Promise<unknown> {
  return nativePlayerCommands.command({ type: 'attach', preferences: {
    repeat: readLegacyRepeat(), ...readLegacyShuffle(),
  } })
}

export const nativeAudioTransport: AudioTransportPort = {
  state: listener => onNativeAudioState(listener),
  event: listener => onNativeAudioEvent(listener),
  snapshot: attachPlayer,
  load: request => command(() => api.nativeAudioLoad(request)),
  unload: () => command(() => api.nativeAudioUnload()),
  play: () => nativePlayerCommands.command({ type: 'playing', playing: true }),
  pause: () => nativePlayerCommands.command({ type: 'playing', playing: false }),
  seek: (seconds, attemptId) => command(() => api.nativeAudioSeek(seconds, attemptId)),
  volume: percent => command(() => api.nativeAudioSetVolume(percent)),
}
