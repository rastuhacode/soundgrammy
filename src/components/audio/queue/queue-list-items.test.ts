import { describe, expect, it } from 'vitest'
import {
  buildQueueListItems,
  estimateQueueListItemSize,
  QUEUE_HEADER_HEIGHT,
  QUEUE_ROW_HEIGHT,
} from './queue-list-items'

describe('buildQueueListItems', () => {
  it('includes history, now, and up next when all open', () => {
    expect(buildQueueListItems({
      trackCount: 5,
      cursor: 2,
      historyOpen: true,
      upNextOpen: true,
    })).toEqual([
      { type: 'header', section: 'history', count: 2 },
      { type: 'track', queueIndex: 0 },
      { type: 'track', queueIndex: 1 },
      { type: 'header', section: 'now', count: 1 },
      { type: 'track', queueIndex: 2 },
      { type: 'header', section: 'upNext', count: 2 },
      { type: 'track', queueIndex: 3 },
      { type: 'track', queueIndex: 4 },
    ])
  })

  it('omits history tracks when history is collapsed', () => {
    expect(buildQueueListItems({
      trackCount: 4,
      cursor: 1,
      historyOpen: false,
      upNextOpen: true,
    })).toEqual([
      { type: 'header', section: 'history', count: 1 },
      { type: 'header', section: 'now', count: 1 },
      { type: 'track', queueIndex: 1 },
      { type: 'header', section: 'upNext', count: 2 },
      { type: 'track', queueIndex: 2 },
      { type: 'track', queueIndex: 3 },
    ])
  })

  it('omits up next tracks when up next is collapsed', () => {
    expect(buildQueueListItems({
      trackCount: 4,
      cursor: 1,
      historyOpen: true,
      upNextOpen: false,
    })).toEqual([
      { type: 'header', section: 'history', count: 1 },
      { type: 'track', queueIndex: 0 },
      { type: 'header', section: 'now', count: 1 },
      { type: 'track', queueIndex: 1 },
      { type: 'header', section: 'upNext', count: 2 },
    ])
  })

  it('skips empty history and up next headers', () => {
    expect(buildQueueListItems({
      trackCount: 1,
      cursor: 0,
      historyOpen: true,
      upNextOpen: true,
    })).toEqual([
      { type: 'header', section: 'now', count: 1 },
      { type: 'track', queueIndex: 0 },
    ])
  })
})

describe('estimateQueueListItemSize', () => {
  it('sizes headers and rows differently', () => {
    expect(estimateQueueListItemSize({ type: 'header', section: 'now', count: 1 }))
      .toBe(QUEUE_HEADER_HEIGHT)
    expect(estimateQueueListItemSize({ type: 'track', queueIndex: 0 }))
      .toBe(QUEUE_ROW_HEIGHT)
  })
})
