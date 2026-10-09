import type { Track } from '@/lib/db'
import type {
  PlaylistQueueEntry,
} from './model'

export type {
  PlaylistQueueEntry,
  ShuffleMode,
  ShuffleState,
} from './model'
export { isShuffleMode, isShuffleState } from './model'
export { SHUFFLE_MODE_OPTIONS } from './modes'

export function buildPlaylistEntries(tracks: Track[]): PlaylistQueueEntry[] {
  return tracks.map((track, sourceIndex) => ({ track, sourceIndex }))
}
