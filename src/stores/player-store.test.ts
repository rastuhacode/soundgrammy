import { beforeEach, describe, expect, it } from 'vitest'
import type { Track } from '@/lib/db'
import {
  clearPendingListenEndReason,
  takePendingListenEndReason,
} from '@/lib/listen-tracker'
import { usePlayerStore } from './player-store'
import { useShuffleStore } from './shuffle-store'

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
  useShuffleStore.setState({ shuffle: 'off', mode: 'random' })
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

describe('shuffle modes', () => {
  it('keeps the shared shuffle state in sync with an explicit playlist action', () => {
    const tracks = [track(1), track(2), track(3)]

    usePlayerStore.getState().playPlaylist({
      id: 7,
      name: 'Mix',
      trackIds: tracks.map(item => item.id),
      tracks,
      isCustom: true,
    }, { shuffle: 'on' })

    expect(useShuffleStore.getState().shuffle).toBe('on')
    expect(usePlayerStore.getState().queue.baseEntries).toHaveLength(3)
    expect(usePlayerStore.getState().queue.cursor).toBe(0)
  })

  it('reconstructs the queue and pins the current membership when the mode changes', () => {
    const a = track(1)
    const b = track(2)
    const c = track(3)
    const baseEntries = [a, b, c].map((item, sourceIndex) => ({
      track: item,
      sourceIndex,
    }))
    usePlayerStore.setState({
      queue: {
        source: {
          type: 'playlist',
          playlistId: 7,
          name: 'Mix',
          trackIds: [1, 2, 3],
        },
        tracks: [a, b, c],
        cursor: 1,
        sourceIndices: [0, 1, 2],
        baseEntries,
      },
      currentTrack: b,
      isPlaying: true,
    })

    usePlayerStore.getState().setShuffleMode('fresh')

    const shuffled = usePlayerStore.getState()
    expect(useShuffleStore.getState()).toMatchObject({
      shuffle: 'on',
      mode: 'fresh',
    })
    expect(shuffled.currentTrack).toBe(b)
    expect(shuffled.queue.cursor).toBe(0)
    expect(shuffled.queue.sourceIndices?.[0]).toBe(1)
    expect(shuffled.queue.baseEntries).toBe(baseEntries)
    expect(shuffled.queue.tracks.map(item => item.id).sort()).toEqual([1, 2, 3])

    usePlayerStore.getState().setShuffle('off')
    const restored = usePlayerStore.getState()
    expect(restored.queue.tracks).toEqual([a, b, c])
    expect(restored.queue.cursor).toBe(1)
    expect(restored.currentTrack).toBe(b)
  })
})
