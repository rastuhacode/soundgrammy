import { captureSession, isSessionCurrent } from '@/stores/session-store'
import { ownEventListeners } from '@/lib/events'
import { errorMessage } from '@/lib/errors'
import { create } from 'zustand'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import {
  api,
  onCacheTracksProgress,
  onPlaylistDownloadProgress,
} from '@/lib/api'
import type { PlaylistDownloadResult } from '@/types'
import type { PlaylistId } from '@/stores/playlists-store'
import { useCacheStore } from '@/stores/cache-store'

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

  runDownloadPlaylist: async ({ playlistId, name, trackIds }) => {
    if (trackIds.length === 0) return
    const key = playlistJobKey(playlistId)
    if (get().downloadJobByPlaylist[key]) return

    const generation = captureSession()
    const jobId = newJobId()
    const job: PlaylistJob = {
      jobId,
      playlistKey: key,
      playlistName: name,
      kind: 'download',
      progress: { current: 0, total: trackIds.length },
      trackIds,
    }

    set(state => ({
      jobsById: { ...state.jobsById, [jobId]: job },
      downloadJobByPlaylist: { ...state.downloadJobByPlaylist, [key]: jobId },
    }))
    useCacheStore.getState().markBusy(trackIds)

    try {
      const result = await api.downloadPlaylist(name, trackIds, jobId)
      if (!isSessionCurrent(generation)) return
      get().enqueueResult({ playlistName: name, result })
      if (result.folderPath) {
        try {
          await revealItemInDir(result.folderPath)
        }
        catch {
          // Result dialog can still open the folder.
        }
      }
    }
    catch (error) {
      if (isSessionCurrent(generation)) get().enqueueError(errorMessage(error))
    }
    finally {
      if (isSessionCurrent(generation)) {
        useCacheStore.getState().clearBusy(trackIds)
        set((state) => {
          const jobsById = { ...state.jobsById }
          delete jobsById[jobId]
          const downloadJobByPlaylist = { ...state.downloadJobByPlaylist }
          delete downloadJobByPlaylist[key]
          return { jobsById, downloadJobByPlaylist }
        })
      }
    }
  },

  runCachePlaylist: async ({ playlistId, name, trackIds }) => {
    if (trackIds.length === 0) return
    const key = playlistJobKey(playlistId)
    if (get().cacheJobByPlaylist[key]) return

    const generation = captureSession()
    const jobId = newJobId()
    const job: PlaylistJob = {
      jobId,
      playlistKey: key,
      playlistName: name,
      kind: 'cache',
      progress: { current: 0, total: trackIds.length },
      trackIds,
    }

    set(state => ({
      jobsById: { ...state.jobsById, [jobId]: job },
      cacheJobByPlaylist: { ...state.cacheJobByPlaylist, [key]: jobId },
    }))
    useCacheStore.getState().markBusy(trackIds)

    try {
      const cached = await api.cacheTracks(trackIds, jobId)
      if (isSessionCurrent(generation)) useCacheStore.getState().markCached(cached)
    }
    catch (error) {
      if (isSessionCurrent(generation)) get().enqueueError(errorMessage(error))
    }
    finally {
      if (isSessionCurrent(generation)) {
        useCacheStore.getState().clearBusy(trackIds)
        set((state) => {
          const jobsById = { ...state.jobsById }
          delete jobsById[jobId]
          const cacheJobByPlaylist = { ...state.cacheJobByPlaylist }
          delete cacheJobByPlaylist[key]
          return { jobsById, cacheJobByPlaylist }
        })
      }
    }
  },
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
