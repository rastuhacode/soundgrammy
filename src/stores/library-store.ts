import { create } from 'zustand'
import type { LibraryBatch, LibraryStatus, PositionedTrack, Track } from '@/types'
import { api } from '@/lib/api'
import { errorMessage } from '@/lib/errors'
import { captureSession, isSessionCurrent, useSessionStore } from '@/stores/session-store'

interface LibraryState {
  library: Track[]
  error: string | null
  revision: number
  status: LibraryStatus | null
  positions: ReadonlyMap<number, number | null>
  setLibrary: (tracks: Track[]) => void
  applyBatch: (batch: LibraryBatch) => boolean
  refresh: () => Promise<Track[] | null>
}

function ordered(tracks: Track[], positions: ReadonlyMap<number, number | null>): Track[] {
  return tracks.sort((a, b) => {
    const first = positions.get(a.id) ?? null
    const second = positions.get(b.id) ?? null
    if (first !== second) {
      if (first === null) return -1
      if (second === null) return 1
      return first - second
    }
    return b.created_at.localeCompare(a.created_at) || b.id - a.id
  })
}

export const useLibraryStore = create<LibraryState>((set, get) => ({
  library: [],
  error: null,
  revision: 0,
  status: null,
  positions: new Map(),
  setLibrary: library => set(state => ({
    library, error: null, revision: state.revision + 1, status: null,
    positions: new Map(library.map((track, index) => [track.id, index])),
  })),
  applyBatch: ({ tracks, removedTrackIds, status }) => {
    const state = get()
    if (useSessionStore.getState().session?.tgUserId !== status.tgUserId
      || (state.status && status.revision <= state.status.revision)) return false
    if (status.blocking) {
      set({ library: [], positions: new Map(), status, error: null })
      return true
    }
    const byId = new Map(state.library.map(track => [track.id, track]))
    const positions = new Map(state.positions)
    for (const { track, position } of tracks) {
      byId.set(track.id, track)
      positions.set(track.id, position)
    }
    for (const id of removedTrackIds) {
      byId.delete(id)
      positions.delete(id)
    }
    // Live batches don't cancel a snapshot repair. Its backend revision decides
    // whether it is fresh enough to replace the merged presentation state.
    set({ library: ordered([...byId.values()], positions), positions, status, error: null })
    return true
  },
  refresh: async () => {
    const generation = captureSession()
    const revision = get().revision + 1
    set({ revision })
    const current = () => isSessionCurrent(generation) && get().revision === revision
    try {
      // Keep a complete local metadata index for search, playlists, selection,
      // and playback. Only IPC transfer is paged; rows already render virtually.
      for (let attempt = 0; attempt < 3; attempt++) {
        const entries: PositionedTrack[] = []
        let status: LibraryStatus | null = null
        let offset = 0
        try {
          for (;;) {
            if (!current()) return null
            const page = await api.listTracksPage(offset, status?.revision)
            if (!current()) return null
            const uid = useSessionStore.getState().session?.tgUserId
            if ((uid !== undefined && uid !== page.status.tgUserId)
              || (status && (status.tgUserId !== page.status.tgUserId || status.revision !== page.status.revision))) {
              throw new Error('Library changed while loading pages; retry')
            }
            status = page.status
            entries.push(...page.tracks)
            if (page.nextOffset === null) break
            if (page.nextOffset <= offset || page.tracks.length === 0) throw new Error('Invalid library page cursor')
            offset = page.nextOffset
          }
          if (status && get().status && status.revision < get().status!.revision) {
            throw new Error('Library changed while loading pages; retry')
          }
        }
        catch (error) {
          if (!current()) return null
          if (attempt < 2 && errorMessage(error).includes('Library changed while loading pages')) continue
          throw error
        }
        const tracks = entries.map(entry => entry.track)
        set({ library: tracks, error: null, status,
          positions: new Map(entries.map(({ track, position }) => [track.id, position])) })
        return tracks
      }
      return null
    }
    catch (error) {
      if (current()) set({ error: errorMessage(error) })
      return null
    }
  },
}))
