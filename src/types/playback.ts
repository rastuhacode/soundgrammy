import { z } from 'zod'
import type { Track } from '@/types'
import type { Queue } from '@/stores/player-store'
import type { RepeatState } from '@/lib/repeat'
import type { ShuffleMode, ShuffleState } from '@/lib/shuffle'

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

const trackSchema = z.object({
  id: z.number().int(), tg_user_id: z.number().int(), file_id: z.string(), file_unique_id: z.string(),
  title: z.string().nullable(), performer: z.string().nullable(), duration: z.number().nullable(),
  source: z.string(), mime_type: z.string().nullable(), file_size: z.number().nullable(), created_at: z.string(),
})
export const playbackSessionSchema = z.object({
  revision: z.number().int().nonnegative(), attempt: z.number().int().nonnegative(),
  isPlaying: z.boolean(), endReason: z.string(),
  preferences: z.object({ repeat: z.enum(['none', 'one', 'all']), shuffle: z.enum(['off', 'on']), mode: z.enum(['random', 'variety', 'rediscover', 'smart', 'fresh', 'duration']) }),
  queue: z.object({
    tracks: z.array(trackSchema), cursor: z.number().int(),
    source: z.object({ type: z.literal('playlist'), playlistId: z.union([z.number(), z.enum(['all', 'liked', 'popular', 'recent'])]), name: z.string(), trackIds: z.array(z.number().int()) }).nullable(),
    sourceIndices: z.array(z.number().int().nonnegative()).nullable(),
    baseEntries: z.array(z.object({ track: trackSchema, sourceIndex: z.number().int().nonnegative() })).nullable(),
  }).refine(q => q.tracks.length === 0 ? q.cursor === -1 : q.cursor >= 0 && q.cursor < q.tracks.length),
})
