import { z } from 'zod'
import type { Track } from '@/types'
import { commonPlaylistIdSchema, type PlaylistId } from '@/lib/playlists'
import type { PlaylistQueueEntry } from '@/lib/shuffle/model'
import { RepeatSchema } from '@/lib/repeat/model'
import { ShuffleSchema, ShuffleModeSchema } from '@/lib/shuffle/model'
import { trackSchema } from '@/types'
import type { RepeatState } from '@/lib/repeat'
import type { ShuffleMode, ShuffleState } from '@/lib/shuffle'

export interface QueueSource {
  type: 'playlist'
  playlistId: PlaylistId
  name: string
  trackIds: number[]
}

export interface Queue {
  source: QueueSource | null
  tracks: Track[]
  cursor: number
  /**
   * Parallel to `tracks`: playlist membership index for each queue slot.
   * Null after queue edits diverge from the source playlist.
   */
  sourceIndices: number[] | null
  /**
   * Unshuffled session order (membership-aware), e.g. UI column sort.
   * Shuffle on/off reshuffles / restores this — not raw playlist membership.
   */
  baseEntries: PlaylistQueueEntry[] | null
}

export interface PlaybackPreferences {
  repeat: RepeatState
  shuffle: ShuffleState
  mode: ShuffleMode
}
export interface PlaybackSession {
  revision: number
  queue: Queue
  isPlaying: boolean
  attempt: number
  endReason: string
  preferences: PlaybackPreferences
}
export type PlayerCommand
  = { type: 'attach', preferences?: PlaybackPreferences }
    | { type: 'setQueue', queue: Queue, play: boolean }
    | { type: 'playPlaylist', queue: Queue, shuffle?: ShuffleState, toggleIfCurrent?: boolean, pinStart?: boolean }
    | { type: 'playTrack', track: Track }
    | { type: 'clear' | 'clearUpNext' | 'toggle' | 'next' | 'toggleRepeat' | 'toggleShuffle' }
    | { type: 'insert', tracks: Track[], next: boolean }
    | { type: 'reorder', fromIndex: number, toIndex: number, queueRevision: number }
    | { type: 'remove', indices: number[], queueRevision: number }
    | { type: 'jump', index: number, queueRevision: number }
    | { type: 'realign', playlistId: number | string, tracks: Track[], moveIndices?: [number, number] }
    | { type: 'refresh', tracks: Track[] }
    | { type: 'playing', playing: boolean }
    | { type: 'previous', restart: boolean }
    | { type: 'repeat', repeat: RepeatState }
    | { type: 'shuffle', shuffle: ShuffleState }
    | { type: 'shuffleMode', mode: ShuffleMode }

export const playbackSessionSchema = z.object({
  revision: z.number().int().nonnegative(), attempt: z.number().int().nonnegative(),
  isPlaying: z.boolean(), endReason: z.string(),
  preferences: z.object({ repeat: RepeatSchema, shuffle: ShuffleSchema, mode: ShuffleModeSchema }),
  queue: z.object({
    tracks: z.array(trackSchema), cursor: z.number().int(),
    source: z.object({ type: z.literal('playlist'), playlistId: z.union([z.number(), commonPlaylistIdSchema]), name: z.string(), trackIds: z.array(z.number().int()) }).nullable(),
    sourceIndices: z.array(z.number().int().nonnegative()).nullable(),
    baseEntries: z.array(z.object({ track: trackSchema, sourceIndex: z.number().int().nonnegative() })).nullable(),
  }).refine(q => q.tracks.length === 0 ? q.cursor === -1 : q.cursor >= 0 && q.cursor < q.tracks.length),
})
