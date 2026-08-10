import { create } from 'zustand'
import type { Track } from '@/lib/db'
import { setPendingListenEndReason } from '@/lib/listen-tracker'
import {
  appendToQueue as appendToQueueHelper,
  clearUpNext as clearUpNextHelper,
  enqueueNext as enqueueNextHelper,
  jumpToQueueIndex as jumpToQueueIndexHelper,
  mapCursorAfterReorder,
  realignQueueAfterPlaylistReorder,
  remapSourceIndicesAfterReorder,
  removeFromQueue as removeFromQueueHelper,
  reorderQueue as reorderQueueHelper,
  type QueueSaveScope,
  trackIdsForSaveScope,
} from '@/lib/queue'
import { useRepeatStore } from '@/stores/repeat-store'
import { useShuffleStore } from '@/stores/shuffle-store'
import { useListenStatsStore } from '@/stores/listen-stats-store'
import type { RepeatState } from '@/lib/repeat'
import {
  buildPlaylistEntries,
  shufflePlaylistEntries,
  shufflePlaylistEntriesByMode,
  type PlaylistQueueEntry,
  type ShuffleAlgorithm,
  type ShuffleContext,
  type ShuffleMode,
  type ShuffleState,
} from '@/lib/shuffle'
import type { PlaylistId, ResolvedSelectedPlaylist } from '@/stores/playlists-store'

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
  shuffle?: ShuffleState
  start?: Track
  /** Index into `orderedEntries` (or playlist.tracks when omitted). */
  startIndex?: number
  shuffleAlgorithm?: ShuffleAlgorithm
  /**
   * Playback order with original membership indexes (e.g. UI column sort).
   * When omitted, membership order is used.
   */
  orderedEntries?: PlaylistQueueEntry[]
}

interface PlayPlaylistOptions {
  start?: Track
  startIndex?: number
  shuffle?: ShuffleState
  shuffleAlgorithm?: ShuffleAlgorithm
  orderedEntries?: PlaylistQueueEntry[]
}

interface PlayerState {
  queue: Queue
  currentTrack: Track | null
  isPlaying: boolean
  /** Bumped when next/prev keeps the same track id (duplicate rows / single-track). */
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
  setShuffleMode: (mode: ShuffleMode) => void
  setRepeat: (repeat: RepeatState) => void
  toggleRepeat: () => void
  hydratePreferences: () => void
  refreshQueueTracks: (libraryTracks: Track[]) => void
  playNext: (options?: { reason?: 'skipped' | 'completed' }) => void
  playPrevious: () => void
}

function normalizeCursor(tracks: Track[], cursor: number): number {
  if (tracks.length === 0) return -1
  return Math.min(Math.max(cursor, 0), tracks.length - 1)
}

function getCurrentTrack(queue: Queue): Track | null {
  if (queue.cursor < 0) return null
  return queue.tracks[queue.cursor] ?? null
}

function shuffleContext(): ShuffleContext {
  const stats = useListenStatsStore.getState()
  return {
    statsByTrackId: stats.statsByTrackId,
    statsEnabled: stats.enabled,
    nowMs: Date.now(),
  }
}

function resolveStartCursor(
  tracks: Track[],
  start?: Track,
  startIndex?: number,
): number {
  if (tracks.length === 0) return -1
  if (startIndex !== undefined) return normalizeCursor(tracks, startIndex)
  if (!start) return 0

  const index = tracks.findIndex(track => track.id === start.id)
  return index === -1 ? 0 : index
}

function resolveSourceTracks(queue: Queue): Track[] {
  if (!queue.source) return queue.tracks

  const trackById = new Map(queue.tracks.map(track => [track.id, track]))
  const sourceTracks = queue.source.trackIds
    .map(id => trackById.get(id))
    .filter((track): track is Track => track !== undefined)

  return sourceTracks.length > 0 ? sourceTracks : queue.tracks
}

function applyQueueMutation(
  queue: Queue,
  result: {
    tracks: Track[]
    cursor: number
    clearSource: boolean
  },
): Queue {
  return {
    source: result.clearSource ? null : queue.source,
    tracks: result.tracks,
    cursor: result.cursor,
    sourceIndices: result.clearSource ? null : queue.sourceIndices,
    baseEntries: result.clearSource ? null : queue.baseEntries,
  }
}

/** Restore unshuffled session order, keeping the same membership as now playing. */
function queueFromBaseEntries(
  queue: Queue,
  baseEntries: PlaylistQueueEntry[],
  playingSourceIndex: number | undefined,
): Queue {
  const tracks = baseEntries.map(entry => entry.track)
  const sourceIndices = baseEntries.map(entry => entry.sourceIndex)
  let cursor = 0
  if (playingSourceIndex != null) {
    const mapped = sourceIndices.indexOf(playingSourceIndex)
    cursor = mapped === -1 ? 0 : mapped
  }
  return {
    ...queue,
    tracks,
    sourceIndices,
    baseEntries,
    cursor: normalizeCursor(tracks, cursor),
  }
}

/** Same track id at a new queue row still needs a listen/playback restart signal. */
function sameTrackEpochPatch(
  listenAttemptEpoch: number,
  prevId: number | null | undefined,
  nextTrack: Track | null,
): { listenAttemptEpoch: number } | Record<string, never> {
  if (nextTrack != null && nextTrack.id === prevId) {
    return { listenAttemptEpoch: listenAttemptEpoch + 1 }
  }
  return {}
}

export const usePlayerStore = create<PlayerState>((set, get) => {
  const emptyQueue: Queue = {
    source: null,
    tracks: [],
    cursor: -1,
    sourceIndices: null,
    baseEntries: null,
  }

  return {
    queue: emptyQueue,
    currentTrack: null,
    isPlaying: false,
    listenAttemptEpoch: 0,

    generateQueue: ({
      playlist,
      shuffle,
      start,
      startIndex,
      shuffleAlgorithm,
      orderedEntries,
    }) => {
      const shuffleStore = useShuffleStore.getState()
      const shuffleState = shuffle ?? shuffleStore.shuffle
      const baseEntries = orderedEntries
        ?? buildPlaylistEntries(playlist.tracks)
      const startTrack = start ?? (
        startIndex !== undefined
          ? baseEntries[startIndex]?.track
          : undefined
      )
      const pinMembershipIndex = startIndex !== undefined
        ? baseEntries[startIndex]?.sourceIndex
        : startTrack
          ? baseEntries.find(entry => entry.track.id === startTrack.id)
            ?.sourceIndex
          : undefined

      const entries = shuffleState === 'on'
        ? shuffleAlgorithm
          ? shufflePlaylistEntries(
              baseEntries,
              shuffleAlgorithm,
              pinMembershipIndex,
            )
          : shufflePlaylistEntriesByMode(
              baseEntries,
              shuffleStore.mode,
              shuffleContext(),
              pinMembershipIndex,
            )
        : baseEntries

      const tracks = entries.map(entry => entry.track)
      const sourceIndices = entries.map(entry => entry.sourceIndex)
      const cursor = shuffleState === 'on'
        ? (tracks.length > 0 ? 0 : -1)
        : resolveStartCursor(tracks, startTrack, startIndex)

      return {
        source: {
          type: 'playlist',
          playlistId: playlist.id,
          name: playlist.name,
          trackIds: playlist.trackIds,
        },
        tracks,
        cursor,
        sourceIndices,
        // Keep pre-shuffle order so shuffle off restores UI sort, not membership.
        baseEntries,
      }
    },

    setQueue: (queue) => {
      const cursor = normalizeCursor(queue.tracks, queue.cursor)
      const nextQueue = { ...queue, cursor }
      set({ queue: nextQueue, currentTrack: getCurrentTrack(nextQueue) })
    },

    clearQueue: () => {
      setPendingListenEndReason('stopped')
      set({
        queue: emptyQueue,
        currentTrack: null,
        isPlaying: false,
      })
    },

    clearUpNext: () => {
      const { queue } = get()
      const result = clearUpNextHelper(queue)
      if (!result.clearSource && result.tracks === queue.tracks) return
      const nextQueue = applyQueueMutation(queue, result)
      set({
        queue: nextQueue,
        currentTrack: getCurrentTrack(nextQueue),
        isPlaying: result.shouldPlay,
      })
    },

    playQueue: (queue, cursor = queue.cursor) => {
      const nextCursor = normalizeCursor(queue.tracks, cursor)
      const nextQueue = { ...queue, cursor: nextCursor }
      const currentTrack = getCurrentTrack(nextQueue)
      const prevId = get().currentTrack?.id
      // Include same-id restarts (duplicate membership / regenerate) so the
      // listen tracker does not fall back to `skipped`.
      if (currentTrack != null && prevId != null) {
        setPendingListenEndReason('replaced')
      }
      set({
        queue: nextQueue,
        currentTrack,
        isPlaying: currentTrack !== null,
        // Same id (e.g. another duplicate membership) must still restart audio.
        ...sameTrackEpochPatch(get().listenAttemptEpoch, prevId, currentTrack),
      })
    },

    playPlaylist: (playlist, options = {}) => {
      const queue = get().generateQueue({ playlist, ...options })
      get().playQueue(queue)
    },

    playTrack: (track) => {
      const { currentTrack, isPlaying, queue } = get()
      if (currentTrack?.id === track.id) {
        set({ isPlaying: !isPlaying })
      }
      else {
        setPendingListenEndReason('replaced')
        const queuedIndex = queue.tracks.findIndex(item => item.id === track.id)
        const nextQueue = queuedIndex === -1
          ? {
              source: null,
              tracks: [track],
              cursor: 0,
              sourceIndices: null,
              baseEntries: null,
            }
          : { ...queue, cursor: queuedIndex }

        set({
          queue: nextQueue,
          currentTrack: getCurrentTrack(nextQueue),
          isPlaying: true,
        })
      }
    },

    enqueueNext: (tracks) => {
      const { queue, isPlaying } = get()
      const wasIdle = queue.tracks.length === 0 || queue.cursor < 0
      const result = enqueueNextHelper(queue, tracks)
      const nextQueue = applyQueueMutation(queue, result)
      if (result.nowPlayingChanged) {
        setPendingListenEndReason('replaced')
      }
      set({
        queue: nextQueue,
        currentTrack: getCurrentTrack(nextQueue),
        // Idle add autoplays; while already in a session, do not resume pause.
        isPlaying: wasIdle ? result.shouldPlay : isPlaying,
      })
    },

    appendToQueue: (tracks) => {
      const { queue, isPlaying } = get()
      const wasIdle = queue.tracks.length === 0 || queue.cursor < 0
      const result = appendToQueueHelper(queue, tracks)
      const nextQueue = applyQueueMutation(queue, result)
      if (result.nowPlayingChanged) {
        setPendingListenEndReason('replaced')
      }
      set({
        queue: nextQueue,
        currentTrack: getCurrentTrack(nextQueue),
        // Idle add autoplays; while already in a session, do not interrupt pause.
        isPlaying: wasIdle ? result.shouldPlay : isPlaying,
      })
    },

    reorderQueue: (fromIndex, toIndex) => {
      const { queue } = get()
      const result = reorderQueueHelper(queue, fromIndex, toIndex)
      if (!result.clearSource && result.tracks === queue.tracks) return
      const nextQueue = applyQueueMutation(queue, result)
      set({
        queue: nextQueue,
        currentTrack: getCurrentTrack(nextQueue),
        isPlaying: result.shouldPlay || get().isPlaying,
      })
    },

    removeFromQueue: (indices) => {
      const { queue } = get()
      const result = removeFromQueueHelper(queue, indices)
      if (!result.clearSource && result.tracks === queue.tracks) return
      if (result.nowPlayingChanged) {
        setPendingListenEndReason(
          result.tracks.length === 0 ? 'stopped' : 'skipped',
        )
      }
      const nextQueue = applyQueueMutation(queue, result)
      set({
        queue: nextQueue,
        currentTrack: getCurrentTrack(nextQueue),
        isPlaying: result.shouldPlay,
      })
    },

    jumpToQueueIndex: (index) => {
      const { queue, currentTrack: prevTrack, listenAttemptEpoch } = get()
      const result = jumpToQueueIndexHelper(queue, index)
      if (result.nowPlayingChanged) {
        setPendingListenEndReason('replaced')
      }
      const nextQueue = applyQueueMutation(queue, result)
      const nextTrack = getCurrentTrack(nextQueue)
      set({
        queue: nextQueue,
        currentTrack: nextTrack,
        isPlaying: result.shouldPlay,
        ...(result.nowPlayingChanged
          ? sameTrackEpochPatch(listenAttemptEpoch, prevTrack?.id, nextTrack)
          : {}),
      })
    },

    realignQueueToPlaylist: (playlistId, tracks, move) => {
      const { queue } = get()
      if (queue.source?.type !== 'playlist') return
      if (queue.source.playlistId !== playlistId) return

      const nextTrackIds = tracks.map(track => track.id)
      const shuffle = useShuffleStore.getState().shuffle

      // Shuffled session order stays; remap membership indexes so highlight
      // follows the playing entry after playlist drag (A B C → B A C).
      if (shuffle !== 'off') {
        const remappedIndices = move
          ? remapSourceIndicesAfterReorder(queue.sourceIndices, move)
          : queue.sourceIndices
        const remappedBase = move && queue.baseEntries
          ? queue.baseEntries.map(entry => ({
              track: entry.track,
              sourceIndex: mapCursorAfterReorder(
                entry.sourceIndex,
                move.fromIndex,
                move.toIndex,
              ),
            }))
          : queue.baseEntries
        set({
          queue: {
            ...queue,
            source: { ...queue.source, trackIds: nextTrackIds },
            sourceIndices: remappedIndices,
            baseEntries: remappedBase,
          },
        })
        return
      }

      const aligned = realignQueueAfterPlaylistReorder(queue, tracks, move)
      const baseEntries = aligned.tracks.map((track, index) => ({
        track,
        sourceIndex: index,
      }))
      const nextQueue: Queue = {
        source: { ...queue.source, trackIds: nextTrackIds },
        tracks: aligned.tracks,
        cursor: aligned.cursor,
        sourceIndices: aligned.tracks.map((_, index) => index),
        baseEntries,
      }
      set({
        queue: nextQueue,
        currentTrack: getCurrentTrack(nextQueue),
      })
    },

    trackIdsForSaveScope: (scope) => {
      return trackIdsForSaveScope(get().queue, scope)
    },

    play: () => get().currentTrack && set({ isPlaying: true }),
    pause: () => set({ isPlaying: false }),
    setPlaying: isPlaying => set({ isPlaying }),

    setShuffle: (shuffle) => {
      const shuffleStore = useShuffleStore.getState()
      shuffleStore.setShuffle(shuffle)

      const { queue, currentTrack } = get()
      if (!currentTrack || queue.tracks.length === 0) return

      // Edited queue (no source): shuffle on reshuffles current list; shuffle off keeps it.
      if (!queue.source) {
        if (shuffle === 'off') return
        const entries = shufflePlaylistEntriesByMode(
          buildPlaylistEntries(queue.tracks),
          shuffleStore.mode,
          shuffleContext(),
          queue.cursor,
        )
        const nextQueue = {
          ...queue,
          tracks: entries.map(entry => entry.track),
          cursor: 0,
          sourceIndices: null,
          baseEntries: null,
        }
        set({ queue: nextQueue, currentTrack: getCurrentTrack(nextQueue) })
        return
      }

      const playingSourceIndex = queue.sourceIndices?.[queue.cursor]
      const baseEntries = queue.baseEntries
        ?? buildPlaylistEntries(resolveSourceTracks(queue))

      if (shuffle === 'on') {
        const entries = shufflePlaylistEntriesByMode(
          baseEntries,
          shuffleStore.mode,
          shuffleContext(),
          playingSourceIndex,
        )
        const nextQueue: Queue = {
          ...queue,
          tracks: entries.map(entry => entry.track),
          sourceIndices: entries.map(entry => entry.sourceIndex),
          baseEntries,
          cursor: 0,
        }
        set({ queue: nextQueue, currentTrack: getCurrentTrack(nextQueue) })
        return
      }

      const nextQueue = queueFromBaseEntries(
        queue,
        baseEntries,
        playingSourceIndex,
      )
      set({ queue: nextQueue, currentTrack: getCurrentTrack(nextQueue) })
    },
    setShuffleMode: (mode) => {
      useShuffleStore.getState().setMode(mode)
      // Reuse the existing path so changing modes reconstructs the queue and
      // pins the currently playing membership at the beginning.
      get().setShuffle('on')
    },

    setRepeat: repeat => useRepeatStore.getState().setRepeat(repeat),
    toggleRepeat: () => useRepeatStore.getState().toggle(),

    hydratePreferences: () => {
      useRepeatStore.getState().hydrate()
      useShuffleStore.getState().hydrate()
    },

    refreshQueueTracks: (libraryTracks) => {
      const { queue } = get()
      if (queue.tracks.length === 0) return

      const trackById = new Map(libraryTracks.map(track => [track.id, track]))
      const playingSourceIndex = queue.sourceIndices?.[queue.cursor] ?? null
      const refreshedPairs = queue.tracks
        .map((track, index) => {
          const next = trackById.get(track.id)
          if (!next) return null
          return {
            track: next,
            sourceIndex: queue.sourceIndices?.[index],
          }
        })
        .filter((pair): pair is { track: Track, sourceIndex: number | undefined } =>
          pair !== null,
        )

      if (refreshedPairs.length === 0) return get().clearQueue()

      const refreshedTracks = refreshedPairs.map(pair => pair.track)
      const nextSourceIndices = queue.sourceIndices
        ? refreshedPairs.map(pair => pair.sourceIndex ?? -1).filter(index => index >= 0)
        : null
      const sourceIndices
        = nextSourceIndices && nextSourceIndices.length === refreshedTracks.length
          ? nextSourceIndices
          : null

      let nextCursor: number
      if (playingSourceIndex != null && sourceIndices) {
        const mapped = sourceIndices.indexOf(playingSourceIndex)
        nextCursor = mapped === -1 ? queue.cursor : mapped
      }
      else {
        const currentTrackId = getCurrentTrack(queue)?.id
        const sameCurrentIndex = currentTrackId === undefined
          ? -1
          : refreshedTracks.findIndex(track => track.id === currentTrackId)
        nextCursor = sameCurrentIndex === -1 ? queue.cursor : sameCurrentIndex
      }

      const nextBaseEntries = queue.baseEntries
        ?.map((entry) => {
          const track = trackById.get(entry.track.id)
          return track
            ? { track, sourceIndex: entry.sourceIndex }
            : null
        })
        .filter((entry): entry is PlaylistQueueEntry => entry !== null)
        ?? null

      const nextQueue: Queue = {
        ...queue,
        tracks: refreshedTracks,
        cursor: normalizeCursor(refreshedTracks, nextCursor),
        sourceIndices,
        baseEntries: nextBaseEntries && nextBaseEntries.length > 0
          ? nextBaseEntries
          : null,
        source: queue.source
          ? {
              ...queue.source,
              trackIds: queue.source.trackIds.filter(id => trackById.has(id)),
            }
          : null,
      }

      set({ queue: nextQueue, currentTrack: getCurrentTrack(nextQueue) })
    },

    playNext: (options) => {
      const reason = options?.reason ?? 'skipped'
      const { queue, listenAttemptEpoch } = get()
      const { repeat } = useRepeatStore.getState()
      if (queue.tracks.length === 0 || queue.cursor < 0) return

      const isLast = queue.cursor === queue.tracks.length - 1
      if (isLast && repeat === 'none') return set({ isPlaying: false })
      const nextQueue = {
        ...queue,
        cursor: isLast ? 0 : queue.cursor + 1,
      }
      const nextTrack = getCurrentTrack(nextQueue)
      const prevId = get().currentTrack?.id
      const sameTrack = nextTrack != null && nextTrack.id === prevId

      if (reason === 'skipped') {
        setPendingListenEndReason('skipped')
      }

      set({
        queue: nextQueue,
        currentTrack: nextTrack,
        isPlaying: true,
        // Skipped: audio + listen tracker need the epoch bump.
        // Completed same-id is restarted in useAudioEngine (avoid double listen).
        ...(reason === 'skipped' && sameTrack
          ? { listenAttemptEpoch: listenAttemptEpoch + 1 }
          : {}),
      })
    },

    playPrevious: () => {
      const { queue, listenAttemptEpoch } = get()
      const { repeat } = useRepeatStore.getState()
      if (queue.tracks.length === 0 || queue.cursor < 0) return
      if (queue.cursor === 0 && repeat === 'none') return
      const nextQueue = {
        ...queue,
        cursor: queue.cursor === 0 ? queue.tracks.length - 1 : queue.cursor - 1,
      }
      const nextTrack = getCurrentTrack(nextQueue)
      const prevId = get().currentTrack?.id

      setPendingListenEndReason('skipped')
      set({
        queue: nextQueue,
        currentTrack: nextTrack,
        isPlaying: true,
        ...sameTrackEpochPatch(listenAttemptEpoch, prevId, nextTrack),
      })
    },
  }
})
