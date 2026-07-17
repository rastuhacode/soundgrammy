import { create } from 'zustand'
import type { Track } from '@/lib/db'
import { setPendingListenEndReason } from '@/lib/listen-tracker'
import { useRepeatStore } from '@/stores/repeat-store'
import { useShuffleStore } from '@/stores/shuffle-store'
import type { RepeatState } from '@/lib/repeat'
import type { ShuffleAlgorithm, ShuffleState } from '@/lib/shuffle'
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
}

interface GenerateQueueOptions {
  playlist: ResolvedSelectedPlaylist
  shuffle?: ShuffleState
  start?: Track
  startIndex?: number
  shuffleAlgorithm?: ShuffleAlgorithm
}

interface PlayPlaylistOptions {
  start?: Track
  startIndex?: number
  shuffle?: ShuffleState
  shuffleAlgorithm?: ShuffleAlgorithm
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
  playQueue: (queue: Queue, cursor?: number) => void
  playPlaylist: (
    playlist: ResolvedSelectedPlaylist,
    options?: PlayPlaylistOptions,
  ) => void
  playTrack: (track: Track) => void
  play: () => void
  pause: () => void
  setPlaying: (playing: boolean) => void
  setShuffle: (shuffle: ShuffleState) => void
  setShuffleAlgorithm: (algorithm: ShuffleAlgorithm) => void
  toggleShuffle: () => void
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

export const usePlayerStore = create<PlayerState>((set, get) => {
  const emptyQueue: Queue = {
    source: null,
    tracks: [],
    cursor: -1,
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
    }) => {
      const shuffleStore = useShuffleStore.getState()
      const shuffleState = shuffle ?? shuffleStore.shuffle
      const startTrack = start ?? (
        startIndex !== undefined ? playlist.tracks[startIndex] : undefined
      )
      const tracks = shuffleState === 'on'
        ? shuffleStore.process(
            playlist.tracks,
            startTrack?.id,
            shuffleAlgorithm,
            shuffleState,
          )
        : [...playlist.tracks]
      const cursor = resolveStartCursor(
        tracks,
        startTrack,
        shuffleState === 'off' ? startIndex : undefined,
      )

      return {
        source: {
          type: 'playlist',
          playlistId: playlist.id,
          name: playlist.name,
          trackIds: playlist.trackIds,
        },
        tracks,
        cursor,
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

    playQueue: (queue, cursor = queue.cursor) => {
      const nextCursor = normalizeCursor(queue.tracks, cursor)
      const nextQueue = { ...queue, cursor: nextCursor }
      const currentTrack = getCurrentTrack(nextQueue)
      const prevId = get().currentTrack?.id
      if (currentTrack && currentTrack.id !== prevId) {
        setPendingListenEndReason('replaced')
      }
      set({
        queue: nextQueue,
        currentTrack,
        isPlaying: currentTrack !== null,
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
          ? { source: null, tracks: [track], cursor: 0 }
          : { ...queue, cursor: queuedIndex }

        set({
          queue: nextQueue,
          currentTrack: getCurrentTrack(nextQueue),
          isPlaying: true,
        })
      }
    },

    play: () => get().currentTrack && set({ isPlaying: true }),
    pause: () => set({ isPlaying: false }),
    setPlaying: isPlaying => set({ isPlaying }),

    setShuffle: (shuffle) => {
      const shuffleStore = useShuffleStore.getState()
      shuffleStore.setShuffle(shuffle)

      const { queue, currentTrack } = get()
      if (!currentTrack || queue.tracks.length === 0) return

      const sourceTracks = resolveSourceTracks(queue)
      const nextTracks = shuffle === 'on'
        ? shuffleStore.process(sourceTracks, currentTrack.id, undefined, shuffle)
        : sourceTracks
      const nextCursor = shuffle === 'on'
        ? 0
        : resolveStartCursor(nextTracks, currentTrack)

      const nextQueue = {
        ...queue,
        tracks: nextTracks,
        cursor: nextCursor,
      }

      set({ queue: nextQueue, currentTrack: getCurrentTrack(nextQueue) })
    },
    setShuffleAlgorithm: algorithm => useShuffleStore.getState().setAlgorithm(algorithm),
    toggleShuffle: () => {
      const { shuffle } = useShuffleStore.getState()
      get().setShuffle(shuffle === 'off' ? 'on' : 'off')
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
      const currentTrackId = getCurrentTrack(queue)?.id
      const refreshedTracks = queue.tracks
        .map(track => trackById.get(track.id))
        .filter((track): track is Track => track !== undefined)

      if (refreshedTracks.length === 0) return get().clearQueue()

      const sameCurrentIndex = currentTrackId === undefined
        ? -1
        : refreshedTracks.findIndex(track => track.id === currentTrackId)
      const nextCursor = normalizeCursor(
        refreshedTracks,
        sameCurrentIndex === -1 ? queue.cursor : sameCurrentIndex,
      )
      const nextQueue = {
        ...queue,
        tracks: refreshedTracks,
        cursor: nextCursor,
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
      const { queue } = get()
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
        ...(reason === 'skipped' && sameTrack
          ? { listenAttemptEpoch: get().listenAttemptEpoch + 1 }
          : {}),
      })
    },

    playPrevious: () => {
      const { queue } = get()
      const { repeat } = useRepeatStore.getState()
      if (queue.tracks.length === 0 || queue.cursor < 0) return
      if (queue.cursor === 0 && repeat === 'none') return

      const nextQueue = {
        ...queue,
        cursor: queue.cursor === 0 ? queue.tracks.length - 1 : queue.cursor - 1,
      }
      const nextTrack = getCurrentTrack(nextQueue)
      const prevId = get().currentTrack?.id
      const sameTrack = nextTrack != null && nextTrack.id === prevId

      setPendingListenEndReason('skipped')
      set({
        queue: nextQueue,
        currentTrack: nextTrack,
        isPlaying: true,
        ...(sameTrack
          ? { listenAttemptEpoch: get().listenAttemptEpoch + 1 }
          : {}),
      })
    },
  }
})
