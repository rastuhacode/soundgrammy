import {
  ALL_TRACKS_PLAYLIST_ID,
  LIKED_PLAYLIST_ID,
  type PlaylistId,
} from '@/stores/playlists-store'

export type PlaylistSortMode = 'recency' | 'custom' | 'alphabetical'

export const PLAYLIST_SORT_MODE_LABELS: Record<PlaylistSortMode, string> = {
  recency: 'Recency',
  custom: 'Custom Order',
  alphabetical: 'Alphabetical',
}

const SORT_MODE_KEY = 'soundgrammy:playlistSortMode'
const SORT_REVERSED_KEY = 'soundgrammy:playlistSortReversed'
const CUSTOM_ORDER_KEY = 'soundgrammy:playlistCustomOrder'

function isPlaylistSortMode(value: string): value is PlaylistSortMode {
  return value === 'recency' || value === 'custom' || value === 'alphabetical'
}

function parsePlaylistId(value: unknown): PlaylistId | null {
  if (value === ALL_TRACKS_PLAYLIST_ID || value === LIKED_PLAYLIST_ID) {
    return value
  }
  if (typeof value === 'number' && Number.isInteger(value) && value > 0) {
    return value
  }
  if (typeof value === 'string') {
    const parsed = Number(value)
    if (Number.isInteger(parsed) && parsed > 0) {
      return parsed
    }
  }
  return null
}

export function readSortMode(): PlaylistSortMode {
  if (typeof window === 'undefined') return 'recency'
  const stored = localStorage.getItem(SORT_MODE_KEY)
  if (stored && isPlaylistSortMode(stored)) return stored
  return 'recency'
}

export function writeSortMode(mode: PlaylistSortMode) {
  if (typeof window === 'undefined') return
  localStorage.setItem(SORT_MODE_KEY, mode)
}

export function readSortReversed(): boolean {
  if (typeof window === 'undefined') return false
  return localStorage.getItem(SORT_REVERSED_KEY) === 'true'
}

export function writeSortReversed(reversed: boolean) {
  if (typeof window === 'undefined') return
  localStorage.setItem(SORT_REVERSED_KEY, String(reversed))
}

export function readCustomOrder(): PlaylistId[] | null {
  if (typeof window === 'undefined') return null
  const stored = localStorage.getItem(CUSTOM_ORDER_KEY)
  if (!stored) return null
  try {
    const parsed = JSON.parse(stored) as unknown
    if (!Array.isArray(parsed)) return null
    const ids = parsed
      .map(parsePlaylistId)
      .filter((id): id is PlaylistId => id !== null)
    return ids.length > 0 ? ids : null
  }
  catch {
    return null
  }
}

export function writeCustomOrder(order: PlaylistId[]) {
  if (typeof window === 'undefined') return
  localStorage.setItem(CUSTOM_ORDER_KEY, JSON.stringify(order))
}

export function defaultCustomOrder(customIds: number[]): PlaylistId[] {
  return [ALL_TRACKS_PLAYLIST_ID, LIKED_PLAYLIST_ID, ...customIds]
}

/** Keep known IDs in saved order, drop missing, append new custom IDs at end. */
export function reconcileCustomOrder(
  saved: PlaylistId[] | null,
  customIds: number[],
): PlaylistId[] {
  const known = new Set<PlaylistId>([
    ALL_TRACKS_PLAYLIST_ID,
    LIKED_PLAYLIST_ID,
    ...customIds,
  ])

  if (!saved) {
    return defaultCustomOrder(customIds)
  }

  const kept = saved.filter(id => known.has(id))
  const keptSet = new Set(kept)
  const missingCustoms = customIds.filter(id => !keptSet.has(id))
  return [...kept, ...missingCustoms]
}

export interface SortablePlaylistItem {
  id: PlaylistId
  name: string
  updatedAt: string
}

export function sortPlaylistItems<T extends SortablePlaylistItem>(
  items: T[],
  mode: PlaylistSortMode,
  reversed: boolean,
  customOrder: PlaylistId[],
): T[] {
  const sorted = [...items]

  if (mode === 'custom') {
    const indexById = new Map(customOrder.map((id, index) => [id, index]))
    sorted.sort((a, b) => {
      const ai = indexById.get(a.id) ?? Number.MAX_SAFE_INTEGER
      const bi = indexById.get(b.id) ?? Number.MAX_SAFE_INTEGER
      return ai - bi
    })
    return sorted
  }

  if (mode === 'alphabetical') {
    sorted.sort((a, b) => a.name.localeCompare(b.name, undefined, {
      sensitivity: 'base',
    }))
  }
  else {
    // recency: newest first
    sorted.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
  }

  if (reversed) {
    sorted.reverse()
  }

  return sorted
}

export function reorderPlaylistIds(
  order: PlaylistId[],
  activeId: PlaylistId,
  overId: PlaylistId,
): PlaylistId[] {
  const oldIndex = order.indexOf(activeId)
  const newIndex = order.indexOf(overId)
  if (oldIndex < 0 || newIndex < 0 || oldIndex === newIndex) {
    return order
  }
  const next = [...order]
  const [moved] = next.splice(oldIndex, 1)
  next.splice(newIndex, 0, moved)
  return next
}

/** Newest library track created_at for All tracks Recency sorting. */
export function libraryUpdatedAt(
  tracks: Array<{ created_at: string }>,
): string {
  if (tracks.length === 0) return ''
  let newest = tracks[0]!.created_at
  for (let i = 1; i < tracks.length; i++) {
    const createdAt = tracks[i]!.created_at
    if (createdAt > newest) newest = createdAt
  }
  return newest
}
