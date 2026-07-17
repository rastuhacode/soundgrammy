import { beforeEach, describe, expect, it } from 'vitest'
import type { Track } from '@/lib/db'
import {
  clearPendingListenEndReason,
  takePendingListenEndReason,
} from '@/lib/listen-tracker'
import { usePlayerStore } from './player-store'

function track(id: number): Track {
  return {
    id,
    tg_user_id: 1,
    file_id: `f${id}`,
    file_unique_id: `u${id}`,
    title: `Title ${id}`,
    performer: `Artist ${id}`,
    duration: id * 10,
    source: 'telegram',
    mime_type: 'audio/mpeg',
    file_size: 1000,
    created_at: '2024-01-01T00:00:00Z',
  }
}

beforeEach(() => {
  clearPendingListenEndReason()
  usePlayerStore.setState({
    queue: {
      source: null,
      tracks: [],
      cursor: -1,
      sourceIndices: null,
      baseEntries: null,
    },
    currentTrack: null,
    isPlaying: false,
    listenAttemptEpoch: 0,
  })
})

describe('enqueueNext', () => {
  it('does not resume playback when the session is paused', () => {
    const a = track(1)
    const b = track(2)
    usePlayerStore.setState({
      queue: {
        source: null,
        tracks: [a],
        cursor: 0,
        sourceIndices: null,
        baseEntries: null,
      },
      currentTrack: a,
      isPlaying: false,
    })

    usePlayerStore.getState().enqueueNext([b])

    const state = usePlayerStore.getState()
    expect(state.isPlaying).toBe(false)
    expect(state.queue.tracks.map(t => t.id)).toEqual([1, 2])
  })
})

describe('playQueue', () => {
  it('marks same-id duplicate jump as replaced for the listen tracker', () => {
    const a = track(1)
    usePlayerStore.setState({
      queue: {
        source: null,
        tracks: [a, a, track(2)],
        cursor: 0,
        sourceIndices: null,
        baseEntries: null,
      },
      currentTrack: a,
      isPlaying: true,
      listenAttemptEpoch: 3,
    })

    usePlayerStore.getState().playQueue(
      {
        source: null,
        tracks: [a, a, track(2)],
        cursor: 1,
        sourceIndices: null,
        baseEntries: null,
      },
      1,
    )

    expect(takePendingListenEndReason('skipped')).toBe('replaced')
    expect(usePlayerStore.getState().listenAttemptEpoch).toBe(4)
  })
})
