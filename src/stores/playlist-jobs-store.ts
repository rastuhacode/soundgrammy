import { captureSession, isSessionCurrent } from '@/stores/session-store'
import { ownEventListeners } from '@/lib/events'
import { errorMessage } from '@/lib/errors'
import { create } from 'zustand'
import {
  api,
  onCacheTracksProgress,
  onPlaylistDownloadProgress,
} from '@/lib/api'
import type { PlaylistDownloadResult } from '@/types'
import type { PlaylistId } from '@/lib/playlists'
import { useCacheStore, withBusyTracks } from '@/stores/cache-store'

export type PlaylistJobKey = string

export interface PlaylistJobProgress {
  current: number
  total: number
}

export interface PlaylistDownloadResultItem {
  playlistName: string
  result: PlaylistDownloadResult
}

interface PlaylistJob {
  jobId: string
  playlistKey: PlaylistJobKey
  playlistName: string
  kind: 'download' | 'cache'
  progress: PlaylistJobProgress | null
  trackIds: number[]
}

interface PlaylistJobsState {
  reset: () => void
  jobsById: Record<string, PlaylistJob>
  downloadJobByPlaylist: Record<PlaylistJobKey, string>
  cacheJobByPlaylist: Record<PlaylistJobKey, string>
  resultQueue: PlaylistDownloadResultItem[]
  errorQueue: string[]

  getDownloadJob: (playlistId: PlaylistId) => PlaylistJob | null
  getCacheJob: (playlistId: PlaylistId) => PlaylistJob | null
  isDownloading: (playlistId: PlaylistId) => boolean
  isCaching: (playlistId: PlaylistId) => boolean

  setJobProgress: (jobId: string, progress: PlaylistJobProgress) => void
  enqueueResult: (item: PlaylistDownloadResultItem) => void
  dismissResult: () => void
  enqueueError: (message: string) => void
  dismissError: () => void

  runDownloadPlaylist: (input: {
    playlistId: PlaylistId
    name: string
    trackIds: number[]
  }) => Promise<void>
  runCachePlaylist: (input: {
    playlistId: PlaylistId
    name: string
    trackIds: number[]
  }) => Promise<void>
}

export function playlistJobKey(playlistId: PlaylistId): PlaylistJobKey {
  return String(playlistId)
}

function newJobId(): string {
  return crypto.randomUUID()
}

interface PlaylistJobInput { playlistId: PlaylistId, name: string, trackIds: number[] }

async function runPlaylistJob(kind: PlaylistJob['kind'], { playlistId, name, trackIds }: PlaylistJobInput) {
  if (!trackIds.length) return
  const store = usePlaylistJobsStore
  const key = playlistJobKey(playlistId)
  const index = kind === 'download' ? 'downloadJobByPlaylist' : 'cacheJobByPlaylist'
  if (store.getState()[index][key]) return
  const generation = captureSession()
  const jobId = newJobId()
  const job: PlaylistJob = { jobId, playlistKey: key, playlistName: name, kind,
    progress: { current: 0, total: trackIds.length }, trackIds }
  store.setState(state => ({ jobsById: { ...state.jobsById, [jobId]: job },
    [index]: { ...state[index], [key]: jobId } }))
  try {
    await withBusyTracks(trackIds, async () => {
      if (kind === 'download') {
        const result = await api.downloadPlaylist(name, trackIds, jobId)
        if (isSessionCurrent(generation)) store.getState().enqueueResult({ playlistName: name, result })
      }
      else {
        const cached = await api.cacheTracks(trackIds, jobId)
        if (isSessionCurrent(generation)) useCacheStore.getState().markCached(cached)
      }
    })
  }
  catch (error) {
    if (isSessionCurrent(generation)) store.getState().enqueueError(errorMessage(error))
  }
  finally {
    if (isSessionCurrent(generation)) {
      store.setState((state) => {
        const jobsById = { ...state.jobsById }
        const byPlaylist = { ...state[index] }
        delete jobsById[jobId]
        delete byPlaylist[key]
        return { jobsById, [index]: byPlaylist }
      })
    }
  }
}

export const usePlaylistJobsStore = create<PlaylistJobsState>((set, get) => ({
  reset: () => set({ jobsById: {}, downloadJobByPlaylist: {}, cacheJobByPlaylist: {}, resultQueue: [], errorQueue: [] }),
  jobsById: {},
  downloadJobByPlaylist: {},
  cacheJobByPlaylist: {},
  resultQueue: [],
  errorQueue: [],

  getDownloadJob: (playlistId) => {
    const jobId = get().downloadJobByPlaylist[playlistJobKey(playlistId)]
    return jobId ? get().jobsById[jobId] ?? null : null
  },

  getCacheJob: (playlistId) => {
    const jobId = get().cacheJobByPlaylist[playlistJobKey(playlistId)]
    return jobId ? get().jobsById[jobId] ?? null : null
  },

  isDownloading: playlistId => get().getDownloadJob(playlistId) != null,
  isCaching: playlistId => get().getCacheJob(playlistId) != null,

  setJobProgress: (jobId, progress) => {
    set((state) => {
      const job = state.jobsById[jobId]
      if (!job) return state
      return {
        jobsById: {
          ...state.jobsById,
          [jobId]: { ...job, progress },
        },
      }
    })
  },

  enqueueResult: (item) => {
    set(state => ({ resultQueue: [...state.resultQueue, item] }))
  },

  dismissResult: () => {
    set((state) => {
      if (state.resultQueue.length === 0) return state
      return { resultQueue: state.resultQueue.slice(1) }
    })
  },

  enqueueError: (message) => {
    set(state => ({ errorQueue: [...state.errorQueue, message] }))
  },

  dismissError: () => {
    set((state) => {
      if (state.errorQueue.length === 0) return state
      return { errorQueue: state.errorQueue.slice(1) }
    })
  },

  runDownloadPlaylist: input => runPlaylistJob('download', input),
  runCachePlaylist: input => runPlaylistJob('cache', input),
}))

export function startPlaylistJobsListeners(): Promise<() => void> {
  const generation = captureSession()
  return Promise.resolve(ownEventListeners([
    onPlaylistDownloadProgress((progress) => {
      if (!isSessionCurrent(generation)) return
      usePlaylistJobsStore.getState().setJobProgress(progress.jobId, {
        current: progress.current,
        total: progress.total,
      })
    }),
    onCacheTracksProgress((progress) => {
      if (!isSessionCurrent(generation)) return
      usePlaylistJobsStore.getState().setJobProgress(progress.jobId, {
        current: progress.current,
        total: progress.total,
      })
    }),
  ]))
}
