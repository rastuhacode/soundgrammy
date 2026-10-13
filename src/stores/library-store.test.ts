import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '@/lib/api'
import { deferred, libraryPage, track } from '@/test-support/fixtures'
import type { LibraryBatch, LibraryPage } from '@/types'
import { useLibraryStore } from './library-store'
import { useSessionStore } from './session-store'

vi.mock('@/lib/api', () => ({ api: { listTracksPage: vi.fn() } }))
beforeEach(() => {
  vi.resetAllMocks()
  useSessionStore.getState().clearSession()
  useSessionStore.getState().setSession({ tgUserId: 1, firstName: 'Test', lastName: null, username: null })
  useLibraryStore.getState().setLibrary([])
})
const batch = (revision: number, ids: number[], removedTrackIds: number[] = []): LibraryBatch => ({
  tracks: ids.map((id, position) => ({ track: track(id), position })), removedTrackIds,
  status: { ...libraryPage([]).status, revision, indexed: ids.length, total: 1000, complete: false },
})

describe('paged local library and progressive batches', () => {
  it('exposes full-index progress but never its partial metadata', () => {
    useLibraryStore.getState().setLibrary([track(1000)])
    const full = batch(1, [1, 2])
    full.status.blocking = true
    full.status.processed = 2
    useLibraryStore.getState().applyBatch(full)
    expect(useLibraryStore.getState().library).toEqual([])
    expect(useLibraryStore.getState().status?.processed).toBe(2)
    const finished = batch(2, [])
    finished.status.complete = true
    useLibraryStore.getState().applyBatch(finished)
    expect(useLibraryStore.getState().library).toEqual([])
  })
  it('hydrates every local page so distant tracks stay available to all features', async () => {
    const tracks = Array.from({ length: 10_037 }, (_, index) => track(index + 1))
    vi.mocked(api.listTracksPage).mockImplementation(async (offset = 0, revision) => {
      expect(revision).toBe(offset ? 7 : undefined)
      const next = offset + 500 < tracks.length ? offset + 500 : null
      const page = libraryPage(tracks.slice(offset, offset + 500), offset, next, 7)
      page.status.indexed = tracks.length
      page.status.total = tracks.length
      return page
    })
    const started = performance.now()
    await useLibraryStore.getState().refresh()
    console.info(`10,037 metadata rows: frontend index assembled in ${(performance.now() - started).toFixed(1)} ms`)
    expect(api.listTracksPage).toHaveBeenCalledTimes(21)
    expect(useLibraryStore.getState().library).toEqual(tracks)
    expect(useLibraryStore.getState().positions.get(10_037)).toBe(10_036)
  })

  it('preserves the previous complete local snapshot if a later page fails', async () => {
    useLibraryStore.getState().setLibrary([track(9000)])
    vi.mocked(api.listTracksPage).mockResolvedValueOnce(libraryPage([track(1)], 0, 1, 1))
      .mockRejectedValueOnce(new Error('Local read failed'))
    expect(await useLibraryStore.getState().refresh()).toBeNull()
    expect(useLibraryStore.getState().library.map(t => t.id)).toEqual([9000])
    expect(useLibraryStore.getState().error).toBe('Local read failed')
  })

  it('restarts a mixed revision snapshot and does not combine old and new pages', async () => {
    vi.mocked(api.listTracksPage).mockResolvedValueOnce(libraryPage([track(1)], 0, 1, 1))
      .mockResolvedValueOnce(libraryPage([track(2)], 1, null, 2))
      .mockResolvedValueOnce(libraryPage([track(3), track(4)], 0, null, 2))
    await useLibraryStore.getState().refresh()
    expect(api.listTracksPage).toHaveBeenNthCalledWith(3, 0, undefined)
    expect(useLibraryStore.getState().library.map(t => t.id)).toEqual([3, 4])
  })

  it('retries a backend revision rejection from the first page', async () => {
    vi.mocked(api.listTracksPage).mockResolvedValueOnce(libraryPage([track(1)], 0, 1, 1))
      .mockRejectedValueOnce({ message: 'Library changed while loading pages; retry' })
      .mockResolvedValueOnce(libraryPage([track(2)], 0, null, 2))
    await useLibraryStore.getState().refresh()
    expect(useLibraryStore.getState().library.map(t => t.id)).toEqual([2])
    expect(useLibraryStore.getState().error).toBeNull()
  })

  it('merges pages without replacing old metadata and removes only explicit deletions', () => {
    useLibraryStore.getState().setLibrary([track(1000)])
    expect(useLibraryStore.getState().applyBatch(batch(1, [1, 2]))).toBe(true)
    expect(new Set(useLibraryStore.getState().library.map(t => t.id))).toEqual(new Set([1, 2, 1000]))
    const updated = batch(2, [1])
    updated.tracks[0]!.track.title = 'Updated title'
    useLibraryStore.getState().applyBatch(updated)
    expect(useLibraryStore.getState().library.find(t => t.id === 1)?.title).toBe('Updated title')
    useLibraryStore.getState().applyBatch({ ...batch(3, [], [1000]), status: { ...batch(3, []).status, complete: true } })
    expect(useLibraryStore.getState().library.map(t => t.id)).toEqual([1, 2])
    expect(useLibraryStore.getState().status?.complete).toBe(true)
  })

  it('a live batch wins over a snapshot already in flight', async () => {
    const pending = deferred<LibraryPage>()
    vi.mocked(api.listTracksPage).mockReturnValueOnce(pending.promise)
      .mockResolvedValueOnce(libraryPage([track(2)], 0, null, 2))
    const refresh = useLibraryStore.getState().refresh()
    useLibraryStore.getState().applyBatch(batch(2, [2]))
    pending.resolve(libraryPage([track(1)], 0, null, 1))
    expect(await refresh).toEqual([track(2)])
    expect(useLibraryStore.getState().library.map(t => t.id)).toEqual([2])
  })

  it('a late completion event cannot cancel repair of a missed earlier page', async () => {
    useLibraryStore.getState().applyBatch(batch(1, [1]))
    const pending = deferred<LibraryPage>()
    vi.mocked(api.listTracksPage).mockReturnValue(pending.promise)
    const refresh = useLibraryStore.getState().refresh()
    // Page revision 2 was missed. The final event contains no metadata.
    const done = batch(3, [])
    done.status.complete = true
    useLibraryStore.getState().applyBatch(done)
    pending.resolve(libraryPage([track(1), track(2)], 0, null, 3))
    await refresh
    expect(useLibraryStore.getState().library.map(t => t.id)).toEqual([1, 2])
    expect(useLibraryStore.getState().status?.complete).toBe(true)
  })

  it('ignores stale batches, batches for another account, and batches after logout', () => {
    useLibraryStore.getState().applyBatch(batch(2, [2]))
    expect(useLibraryStore.getState().applyBatch(batch(1, [1]))).toBe(false)
    const other = batch(3, [3])
    other.status.tgUserId = 99
    expect(useLibraryStore.getState().applyBatch(other)).toBe(false)
    useSessionStore.getState().clearSession()
    expect(useLibraryStore.getState().applyBatch(batch(3, [3]))).toBe(false)
    expect(useLibraryStore.getState().library.map(t => t.id)).toEqual([2])
  })

  it('does not request more pages after logout while the first page is pending', async () => {
    const pending = deferred<LibraryPage>()
    vi.mocked(api.listTracksPage).mockReturnValue(pending.promise)
    const refresh = useLibraryStore.getState().refresh()
    useSessionStore.getState().clearSession()
    pending.resolve(libraryPage([track(1)], 0, 1, 1))
    await refresh
    expect(api.listTracksPage).toHaveBeenCalledTimes(1)
    expect(useLibraryStore.getState().library).toEqual([])
  })

  it('rejects a nonadvancing cursor without committing a partial index', async () => {
    vi.mocked(api.listTracksPage).mockResolvedValue(libraryPage([track(1)], 0, 0, 1))
    await useLibraryStore.getState().refresh()
    expect(api.listTracksPage).toHaveBeenCalledTimes(1)
    expect(useLibraryStore.getState().library).toEqual([])
    expect(useLibraryStore.getState().error).toBe('Invalid library page cursor')
  })
})
