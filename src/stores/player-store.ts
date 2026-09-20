import { create } from 'zustand'
import { api } from '@/lib/api'
import type { Track } from '@/lib/db'
import { setPendingListenEndReason } from '@/lib/listen-tracker'
import { trackIdsForSaveScope, type QueueSaveScope } from '@/lib/queue'
import { readLegacyRepeat, useRepeatStore } from '@/stores/repeat-store'
import { readLegacyShuffle, useShuffleStore } from '@/stores/shuffle-store'
import type { RepeatState } from '@/lib/repeat'
import { buildPlaylistEntries, type PlaylistQueueEntry, type ShuffleMode, type ShuffleState } from '@/lib/shuffle'
import type { PlaylistId, ResolvedSelectedPlaylist } from '@/stores/playlists-store'
import { playbackSessionSchema, type PlaybackSession, type PlayerCommand } from '@/types/playback'

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

interface GenerateQueueOptions {
  playlist: ResolvedSelectedPlaylist
  start?: Track
  /** Index into `orderedEntries` (or playlist.tracks when omitted). */
  startIndex?: number
  /**
   * Playback order with original membership indexes (e.g. UI column sort).
   * When omitted, membership order is used.
   */
  orderedEntries?: PlaylistQueueEntry[]
}

interface PlayPlaylistOptions {
  toggleIfCurrent?: boolean
  start?: Track
  startIndex?: number
  shuffle?: ShuffleState
  orderedEntries?: PlaylistQueueEntry[]
}

interface PlayerState {
  queue: Queue
  currentTrack: Track | null
  isPlaying: boolean
  /** Native attempt counter; duplicate memberships and replay get fresh attempts. */
  listenAttemptEpoch: number

  generateQueue: (options: GenerateQueueOptions) => Queue
  setQueue: (queue: Queue) => void
  clearQueue: () => void
  clearUpNext: () => void
  playQueue: (queue: Queue, cursor?: number) => void
  playPlaylist: (
    playlist: ResolvedSelectedPlaylist,
    options?: PlayPlaylistOptions,
  ) => void
  playTrack: (track: Track) => void
  enqueueNext: (tracks: Track[]) => void
  appendToQueue: (tracks: Track[]) => void
  reorderQueue: (fromIndex: number, toIndex: number) => void
  removeFromQueue: (indices: number[]) => void
  jumpToQueueIndex: (index: number) => void
  /** Keep session queue aligned when its source playlist membership is reordered. */
  realignQueueToPlaylist: (
    playlistId: PlaylistId,
    tracks: Track[],
    move?: { fromIndex: number, toIndex: number },
  ) => void
  trackIdsForSaveScope: (scope: QueueSaveScope) => number[]
  play: () => void
  pause: () => void
  setPlaying: (playing: boolean) => void
  setShuffle: (shuffle: ShuffleState) => void
  toggleShuffle: () => void
  setShuffleMode: (mode: ShuffleMode) => void
  setRepeat: (repeat: RepeatState) => void
  toggleRepeat: () => void
  refreshQueueTracks: (libraryTracks: Track[]) => void
  playNext: () => void
  playPrevious: () => void
  previousOrRestart: () => void
  togglePlaying: () => void
  nativeRevision: number
  commandError: string | null
}

// Commands express intent; only acknowledged native snapshots mutate playback state.
let commands: Promise<void> = Promise.resolve()
export function sendPlayerCommand(command: PlayerCommand): Promise<void> {
  const run = commands.then(async () => {
    const result = await api.nativePlayerCommand(command)
    acceptPlayerResponse(result)
    usePlayerStore.setState({ commandError: null })
  })
  commands = run.catch(async (error: unknown) => {
    usePlayerStore.setState({ commandError: error instanceof Error ? error.message : String(error) })
    // A rejected stale index command never retries against a different row.
    try {
      acceptPlayerResponse(await api.nativeAudioSnapshot())
    }
    catch { /* Keep the last acknowledged state until reattachment succeeds. */ }
  })
  return commands
}

export function acceptPlayerResponse(value: unknown) {
  if (value && typeof value === 'object' && 'player' in value && value.player) {
    const parsed = playbackSessionSchema.safeParse(value.player)
    if (parsed.success) applyPlaybackSession(parsed.data)
  }
}
export function applyPlaybackSession(session: PlaybackSession) {
  if (session.revision <= usePlayerStore.getState().nativeRevision) return
  if (session.attempt !== usePlayerStore.getState().listenAttemptEpoch && ['completed', 'skipped', 'replaced', 'stopped'].includes(session.endReason)) {
    setPendingListenEndReason(session.endReason as 'completed' | 'skipped' | 'replaced' | 'stopped')
  }
  useRepeatStore.setState({ repeat: session.preferences.repeat })
  useShuffleStore.setState({ shuffle: session.preferences.shuffle, mode: session.preferences.mode })
  usePlayerStore.setState({
    queue: session.queue,
    currentTrack: session.queue.tracks[session.queue.cursor] ?? null,
    isPlaying: session.isPlaying,
    listenAttemptEpoch: session.attempt,
    nativeRevision: session.revision,
  })
}

export async function attachPlayer(): Promise<unknown> {
  return api.nativePlayerCommand({ type: 'attach', preferences: {
    repeat: readLegacyRepeat(), ...readLegacyShuffle(),
  } })
}

export const usePlayerStore = create<PlayerState>((_set, get) => ({
  queue: { source: null, tracks: [], cursor: -1, sourceIndices: null, baseEntries: null },
  currentTrack: null,
  isPlaying: false,
  listenAttemptEpoch: 0,
  nativeRevision: -1,
  commandError: null,
  // UI sorting defines the base membership order. Native policy applies shuffle.
  generateQueue: ({ playlist, start, startIndex, orderedEntries }) => {
    const entries = orderedEntries ?? buildPlaylistEntries(playlist.tracks)
    const index = startIndex ?? (start ? entries.findIndex(e => e.track.id === start.id) : 0)
    return {
      source: { type: 'playlist', playlistId: playlist.id, name: playlist.name, trackIds: playlist.trackIds },
      tracks: entries.map(e => e.track),
      cursor: entries.length ? Math.min(Math.max(index, 0), entries.length - 1) : -1,
      sourceIndices: entries.map(e => e.sourceIndex),
      baseEntries: entries,
    }
  },
  setQueue: (queue) => { void sendPlayerCommand({ type: 'setQueue', queue, play: false }) },
  clearQueue: () => { void sendPlayerCommand({ type: 'clear' }) },
  clearUpNext: () => { void sendPlayerCommand({ type: 'clearUpNext' }) },
  playQueue: (queue, cursor = queue.cursor) => { void sendPlayerCommand({ type: 'setQueue', queue: { ...queue, cursor }, play: true }) },
  playPlaylist: (playlist, options = {}) => { void sendPlayerCommand({ type: 'playPlaylist', queue: get().generateQueue({ playlist, ...options }), shuffle: options.shuffle, toggleIfCurrent: options.toggleIfCurrent, pinStart: options.start !== undefined || options.startIndex !== undefined }) },
  playTrack: (track) => { void sendPlayerCommand({ type: 'playTrack', track }) },
  enqueueNext: (tracks) => { void sendPlayerCommand({ type: 'insert', tracks, next: true }) },
  appendToQueue: (tracks) => { void sendPlayerCommand({ type: 'insert', tracks, next: false }) },
  reorderQueue: (fromIndex, toIndex) => { void sendPlayerCommand({ type: 'reorder', fromIndex, toIndex, queueRevision: get().nativeRevision }) },
  removeFromQueue: (indices) => { void sendPlayerCommand({ type: 'remove', indices, queueRevision: get().nativeRevision }) },
  jumpToQueueIndex: (index) => { void sendPlayerCommand({ type: 'jump', index, queueRevision: get().nativeRevision }) },
  realignQueueToPlaylist: (playlistId, tracks, move) => { void sendPlayerCommand({ type: 'realign', playlistId, tracks, moveIndices: move ? [move.fromIndex, move.toIndex] : undefined }) },
  trackIdsForSaveScope: scope => trackIdsForSaveScope(get().queue, scope),
  play: () => { void sendPlayerCommand({ type: 'playing', playing: true }) },
  pause: () => { void sendPlayerCommand({ type: 'playing', playing: false }) },
  setPlaying: (playing) => { void sendPlayerCommand({ type: 'playing', playing }) },
  togglePlaying: () => { void sendPlayerCommand({ type: 'toggle' }) },
  toggleShuffle: () => { void sendPlayerCommand({ type: 'toggleShuffle' }) },
  setShuffle: (shuffle) => { void sendPlayerCommand({ type: 'shuffle', shuffle }) },
  setShuffleMode: (mode) => { void sendPlayerCommand({ type: 'shuffleMode', mode }) },
  setRepeat: (repeat) => { void sendPlayerCommand({ type: 'repeat', repeat }) },
  toggleRepeat: () => { void sendPlayerCommand({ type: 'toggleRepeat' }) },
  refreshQueueTracks: (tracks) => { void sendPlayerCommand({ type: 'refresh', tracks }) },
  playNext: () => { void sendPlayerCommand({ type: 'next' }) },
  playPrevious: () => { void sendPlayerCommand({ type: 'previous', restart: false }) },
  previousOrRestart: () => { void sendPlayerCommand({ type: 'previous', restart: true }) },
}))
