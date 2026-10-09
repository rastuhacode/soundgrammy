import { assertSession, createSessionQueue, isSessionCurrent } from '@/stores/session-store'
import { create } from 'zustand'
import { nativePlayerCommands } from '@/lib/native-playback'
import { appLogger } from '@/lib/app-logger'
import { contractIssues } from '@/lib/errors'
import type { Track } from '@/lib/db'
import { trackIdsForSaveScope, type QueueSaveScope } from '@/lib/queue'
import { useRepeatStore } from '@/stores/repeat-store'
import { useShuffleStore } from '@/stores/shuffle-store'
import type { RepeatState } from '@/lib/repeat'
import type { PlaylistQueueEntry, ShuffleMode, ShuffleState } from '@/lib/shuffle'
import { playlistEntries, type PlaylistId, type ResolvedSelectedPlaylist } from '@/lib/playlists'
import { playbackSessionSchema, type PlaybackSession, type PlayerCommand, type PlayerCommandPort, type Queue } from '@/types/playback'

export type { Queue, QueueSource } from '@/types/playback'

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
  reset: () => void

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
  ) => Promise<void>
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

// Commands express intent; only acknowledged snapshots mutate playback state.
let commandPort: PlayerCommandPort = nativePlayerCommands
let portRevision = 0
let enqueue = createSessionQueue()

/** Composition supplies commands as well as the observed audio engine. */
export function installPlayerCommands(port: PlayerCommandPort): () => void {
  const revision = ++portRevision
  commandPort = port
  enqueue = createSessionQueue()
  return () => {
    if (revision !== portRevision) return
    ++portRevision
    commandPort = nativePlayerCommands
    enqueue = createSessionQueue()
  }
}

export function sendPlayerCommand(command: PlayerCommand): Promise<void> {
  const port = commandPort
  const revision = portRevision
  return enqueue(async (generation) => {
    if (revision !== portRevision) throw new Error('Playback controls were detached.')
    try {
      const result = await port.command(command)
      assertSession(generation)
      if (revision !== portRevision) throw new Error('Playback controls were detached.')
      acceptPlayerResponse(result)
      usePlayerStore.setState({ commandError: null })
    }
    catch (error) {
      if (!isSessionCurrent(generation) || revision !== portRevision) throw error
      usePlayerStore.setState({ commandError: error instanceof Error ? error.message : String(error) })
      // Refresh presentation; never retry an index edit against a different row.
      try {
        const snapshot = await port.snapshot()
        if (isSessionCurrent(generation) && revision === portRevision) acceptPlayerResponse(snapshot)
      }
      catch { /* Keep the last acknowledged state until reattachment succeeds. */ }
      throw error
    }
  })
}

// UI callbacks report failures through commandError; orchestration can await/reject.
function dispatchPlayerCommand(command: PlayerCommand) {
  void sendPlayerCommand(command).catch(() => {})
}

export function acceptPlayerResponse(value: unknown) {
  if (value && typeof value === 'object' && 'player' in value && value.player) {
    const parsed = playbackSessionSchema.safeParse(value.player)
    if (parsed.success) applyPlaybackSession(parsed.data)
    else appLogger.error({ source: 'audio', title: 'Invalid native player response', context: { issues: contractIssues(parsed.error.issues) } })
  }
}
export function applyPlaybackSession(session: PlaybackSession) {
  if (session.revision <= usePlayerStore.getState().nativeRevision) return
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

export { attachPlayer } from '@/lib/native-playback'

function emptyPlayerMirror(): Pick<PlayerState, 'queue' | 'currentTrack' | 'isPlaying' | 'listenAttemptEpoch' | 'nativeRevision' | 'commandError'> {
  return { queue: { source: null, tracks: [], cursor: -1, sourceIndices: null, baseEntries: null },
    currentTrack: null, isPlaying: false, listenAttemptEpoch: 0, nativeRevision: -1, commandError: null }
}

export const usePlayerStore = create<PlayerState>((set, get) => ({
  ...emptyPlayerMirror(),
  // Logout/revocation already clears the native session; invalidate UI mirrors immediately.
  reset: () => set(emptyPlayerMirror()),
  // UI sorting defines the base membership order. Native policy applies shuffle.
  generateQueue: ({ playlist, start, startIndex, orderedEntries }) => {
    const entries = orderedEntries ?? playlistEntries(playlist)
    const index = startIndex ?? (start ? entries.findIndex(e => e.track.id === start.id) : 0)
    return {
      source: { type: 'playlist', playlistId: playlist.id, name: playlist.name, trackIds: playlist.trackIds },
      tracks: entries.map(e => e.track),
      cursor: entries.length ? Math.min(Math.max(index, 0), entries.length - 1) : -1,
      sourceIndices: entries.map(e => e.sourceIndex),
      baseEntries: entries,
    }
  },
  setQueue: (queue) => { dispatchPlayerCommand({ type: 'setQueue', queue, play: false }) },
  clearQueue: () => { dispatchPlayerCommand({ type: 'clear' }) },
  clearUpNext: () => { dispatchPlayerCommand({ type: 'clearUpNext' }) },
  playQueue: (queue, cursor = queue.cursor) => { dispatchPlayerCommand({ type: 'setQueue', queue: { ...queue, cursor }, play: true }) },
  playPlaylist: (playlist, options = {}) => { dispatchPlayerCommand({ type: 'playPlaylist', queue: get().generateQueue({ playlist, ...options }), shuffle: options.shuffle, toggleIfCurrent: options.toggleIfCurrent, pinStart: options.start !== undefined || options.startIndex !== undefined }) },
  playTrack: (track) => { dispatchPlayerCommand({ type: 'playTrack', track }) },
  enqueueNext: (tracks) => { dispatchPlayerCommand({ type: 'insert', tracks, next: true }) },
  appendToQueue: (tracks) => { dispatchPlayerCommand({ type: 'insert', tracks, next: false }) },
  reorderQueue: (fromIndex, toIndex) => { dispatchPlayerCommand({ type: 'reorder', fromIndex, toIndex, queueRevision: get().nativeRevision }) },
  removeFromQueue: (indices) => { dispatchPlayerCommand({ type: 'remove', indices, queueRevision: get().nativeRevision }) },
  jumpToQueueIndex: (index) => { dispatchPlayerCommand({ type: 'jump', index, queueRevision: get().nativeRevision }) },
  realignQueueToPlaylist: (playlistId, tracks, move) => sendPlayerCommand({ type: 'realign', playlistId, tracks, moveIndices: move ? [move.fromIndex, move.toIndex] : undefined }),
  trackIdsForSaveScope: scope => trackIdsForSaveScope(get().queue, scope),
  play: () => { dispatchPlayerCommand({ type: 'playing', playing: true }) },
  pause: () => { dispatchPlayerCommand({ type: 'playing', playing: false }) },
  setPlaying: (playing) => { dispatchPlayerCommand({ type: 'playing', playing }) },
  togglePlaying: () => { dispatchPlayerCommand({ type: 'toggle' }) },
  toggleShuffle: () => { dispatchPlayerCommand({ type: 'toggleShuffle' }) },
  setShuffle: (shuffle) => { dispatchPlayerCommand({ type: 'shuffle', shuffle }) },
  setShuffleMode: (mode) => { dispatchPlayerCommand({ type: 'shuffleMode', mode }) },
  setRepeat: (repeat) => { dispatchPlayerCommand({ type: 'repeat', repeat }) },
  toggleRepeat: () => { dispatchPlayerCommand({ type: 'toggleRepeat' }) },
  refreshQueueTracks: (tracks) => { dispatchPlayerCommand({ type: 'refresh', tracks }) },
  playNext: () => { dispatchPlayerCommand({ type: 'next' }) },
  playPrevious: () => { dispatchPlayerCommand({ type: 'previous', restart: false }) },
  previousOrRestart: () => { dispatchPlayerCommand({ type: 'previous', restart: true }) },
}))
