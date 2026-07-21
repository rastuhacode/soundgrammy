import { describe, expect, it } from 'vitest'
import {
  ALL_TRACKS_PLAYLIST_ID,
  LIKED_PLAYLIST_ID,
  POPULAR_PLAYLIST_ID,
  RECENT_PLAYLIST_ID,
} from '@/stores/playlists-store'
import {
  defaultCustomOrder,
  reconcileCustomOrder,
} from './playlist-sort'

describe('defaultCustomOrder', () => {
  it('places system playlists before customs', () => {
    expect(defaultCustomOrder([10, 20])).toEqual([
      ALL_TRACKS_PLAYLIST_ID,
      LIKED_PLAYLIST_ID,
      POPULAR_PLAYLIST_ID,
      RECENT_PLAYLIST_ID,
      10,
      20,
    ])
  })
})

describe('reconcileCustomOrder', () => {
  it('returns default when nothing saved', () => {
    expect(reconcileCustomOrder(null, [5])).toEqual(defaultCustomOrder([5]))
  })

  it('keeps saved order and appends new custom ids', () => {
    const saved = [
      ALL_TRACKS_PLAYLIST_ID,
      LIKED_PLAYLIST_ID,
      POPULAR_PLAYLIST_ID,
      RECENT_PLAYLIST_ID,
      1,
    ]
    expect(reconcileCustomOrder(saved, [1, 2])).toEqual([...saved, 2])
  })

  it('inserts missing popular/recent after Liked for older saved orders', () => {
    const saved = [ALL_TRACKS_PLAYLIST_ID, LIKED_PLAYLIST_ID, 7]
    expect(reconcileCustomOrder(saved, [7])).toEqual([
      ALL_TRACKS_PLAYLIST_ID,
      LIKED_PLAYLIST_ID,
      POPULAR_PLAYLIST_ID,
      RECENT_PLAYLIST_ID,
      7,
    ])
  })

  it('drops unknown ids from saved order', () => {
    const saved = [
      ALL_TRACKS_PLAYLIST_ID,
      LIKED_PLAYLIST_ID,
      POPULAR_PLAYLIST_ID,
      RECENT_PLAYLIST_ID,
      99,
    ]
    expect(reconcileCustomOrder(saved, [1])).toEqual([
      ALL_TRACKS_PLAYLIST_ID,
      LIKED_PLAYLIST_ID,
      POPULAR_PLAYLIST_ID,
      RECENT_PLAYLIST_ID,
      1,
    ])
  })
})
