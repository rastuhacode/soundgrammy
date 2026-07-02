import { create } from 'zustand'
import type { Track } from '@/lib/db'
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
  playNext: () => void
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
          trackIds: tracks.map(track => track.id),
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

    clearQueue: () => set({
      queue: emptyQueue,
      currentTrack: null,
      isPlaying: false,
    }),

    playQueue: (queue, cursor = queue.cursor) => {
      const nextCursor = normalizeCursor(queue.tracks, cursor)
      const nextQueue = { ...queue, cursor: nextCursor }
      const currentTrack = getCurrentTrack(nextQueue)
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

    setShuffle: shuffle => useShuffleStore.getState().setShuffle(shuffle),
    setShuffleAlgorithm: algorithm => useShuffleStore.getState().setAlgorithm(algorithm),
    toggleShuffle: () => useShuffleStore.getState().toggle(),

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
          ? { ...queue.source, trackIds: refreshedTracks.map(track => track.id) }
          : null,
      }

      set({ queue: nextQueue, currentTrack: getCurrentTrack(nextQueue) })
    },

    playNext: () => {
      const { queue } = get()
      const { repeat } = useRepeatStore.getState()
      if (queue.tracks.length === 0 || queue.cursor < 0) return

      if (repeat === 'one') return set({ isPlaying: true })

      const isLast = queue.cursor === queue.tracks.length - 1
      if (isLast && repeat === 'none') return set({ isPlaying: false })

      const nextQueue = {
        ...queue,
        cursor: isLast ? 0 : queue.cursor + 1,
      }
      set({
        queue: nextQueue,
        currentTrack: getCurrentTrack(nextQueue),
        isPlaying: true,
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
      set({
        queue: nextQueue,
        currentTrack: getCurrentTrack(nextQueue),
        isPlaying: true,
      })
    },
  }
})
