export type QueueSectionId = 'history' | 'now' | 'upNext'

export type QueueListItem
  = | { type: 'header', section: QueueSectionId, count: number }
    | { type: 'track', queueIndex: number }

export interface BuildQueueListItemsOptions {
  trackCount: number
  cursor: number
  historyOpen: boolean
  upNextOpen: boolean
}

/** Flat list for the virtualized queue popover (collapsed sections omit tracks). */
export function buildQueueListItems(
  options: BuildQueueListItemsOptions,
): QueueListItem[] {
  const { trackCount, cursor, historyOpen, upNextOpen } = options
  if (trackCount === 0 || cursor < 0) return []

  const items: QueueListItem[] = []
  const historyCount = cursor
  const upNextCount = Math.max(0, trackCount - cursor - 1)

  if (historyCount > 0) {
    items.push({ type: 'header', section: 'history', count: historyCount })
    if (historyOpen) {
      for (let i = 0; i < cursor; i++) {
        items.push({ type: 'track', queueIndex: i })
      }
    }
  }

  items.push({ type: 'header', section: 'now', count: 1 })
  items.push({ type: 'track', queueIndex: cursor })

  if (upNextCount > 0) {
    items.push({ type: 'header', section: 'upNext', count: upNextCount })
    if (upNextOpen) {
      for (let i = cursor + 1; i < trackCount; i++) {
        items.push({ type: 'track', queueIndex: i })
      }
    }
  }

  return items
}

export const QUEUE_ROW_HEIGHT = 56
export const QUEUE_HEADER_HEIGHT = 28

export function estimateQueueListItemSize(item: QueueListItem | undefined): number {
  if (!item) return QUEUE_ROW_HEIGHT
  return item.type === 'header' ? QUEUE_HEADER_HEIGHT : QUEUE_ROW_HEIGHT
}
