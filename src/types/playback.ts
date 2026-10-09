import { z } from 'zod'
import type { Track } from '@/types'
import { commonPlaylistIdSchema } from '@/lib/playlists'
import { RepeatSchema } from '@/lib/repeat/model'
import { ShuffleSchema, ShuffleModeSchema } from '@/lib/shuffle/model'
import { trackSchema } from '@/types'
import type { RepeatState } from '@/lib/repeat'
import type { ShuffleMode, ShuffleState } from '@/lib/shuffle'

export type PlaybackSession = z.infer<typeof playbackSessionSchema>
export type Queue = PlaybackSession['queue']
export type QueueSource = NonNullable<Queue['source']>
export type PlaybackPreferences = PlaybackSession['preferences']

/** Command acknowledgement is independent of transport observation. */
export interface PlayerCommandPort {
  command(command: PlayerCommand): Promise<unknown>
  snapshot(): Promise<unknown>
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
    // Parallel to tracks at the IPC boundary; null once queue edits diverge from its source.
    sourceIndices: z.array(z.number().int().nonnegative()).nullable(),
    // Membership-aware order before native shuffle, including UI column sorting.
    baseEntries: z.array(z.object({ track: trackSchema, sourceIndex: z.number().int().nonnegative() })).nullable(),
  }).refine(q => q.tracks.length === 0 ? q.cursor === -1 : q.cursor >= 0 && q.cursor < q.tracks.length),
})
