import {
  indexOfTrackOccurrence,
  trackOccurrenceAtIndex,
} from './index'

/**
 * Which playlist membership row should show as now-playing.
 * Prefers queue.sourceIndices so shuffled duplicates stay distinct.
 */
export function resolvePlayingSourceIndex(options: {
  currentTrackId: number | null
  playlistId: string | number
  playlistTrackIds: number[]
  queue: {
    cursor: number
    source: { type: 'playlist', playlistId: string | number } | null
    sourceIndices: number[] | null
    trackIds: number[]
  }
}): number | null {
  const { currentTrackId, playlistId, playlistTrackIds, queue } = options
  if (currentTrackId == null || queue.cursor < 0) return null

  const fromThisPlaylist = queue.source?.type === 'playlist'
    && queue.source.playlistId === playlistId

  if (fromThisPlaylist) {
    const membership = queue.sourceIndices?.[queue.cursor]
    if (
      membership != null
      && membership >= 0
      && membership < playlistTrackIds.length
      && playlistTrackIds[membership] === currentTrackId
    ) {
      return membership
    }

    const orderMatches = queue.trackIds.length === playlistTrackIds.length
      && queue.trackIds.every((id, index) => id === playlistTrackIds[index])

    if (orderMatches) {
      return queue.cursor < playlistTrackIds.length ? queue.cursor : null
    }
  }

  // After queue edits clear `source` / `sourceIndices`, map via occurrence so
  // a second duplicate stays highlighted (indexOf would always pick the first).
  const playingId = queue.trackIds[queue.cursor]
  if (playingId != null && playingId === currentTrackId) {
    const occurrence = trackOccurrenceAtIndex(queue.trackIds, queue.cursor)
    const index = indexOfTrackOccurrence(playlistTrackIds, playingId, occurrence)
    if (index !== -1) return index
  }

  const firstIndex = playlistTrackIds.indexOf(currentTrackId)
  return firstIndex === -1 ? null : firstIndex
}
