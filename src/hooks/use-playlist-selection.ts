import { useMemo, useState } from 'react'
import type { OnChangeFn, RowSelectionState } from '@tanstack/react-table'
import { playlistEntries, playlistMembershipKey, type PlaylistEntry, type ResolvedSelectedPlaylist } from '@/lib/playlists'

/** Selection belongs to one exact ordered membership snapshot. */
export function usePlaylistSelection(playlist: ResolvedSelectedPlaylist, entries: PlaylistEntry[]) {
  const key = useMemo(() => playlistMembershipKey(playlist), [playlist])
  const [selection, setSelection] = useState<{ key: string, rows: RowSelectionState } | null>(null)
  const mode = selection?.key === key
  const rows = useMemo<RowSelectionState>(() => mode ? selection!.rows : {}, [selection, mode])
  if (selection && !mode) setSelection(null)
  const setRows: OnChangeFn<RowSelectionState> = (update) => {
    setSelection(previous => ({ key, rows: typeof update === 'function'
      ? update(previous?.key === key ? previous.rows : {})
      : update }))
  }
  const selectedEntries = useMemo(() => entries.filter(entry => rows[String(entry.sourceIndex)]), [entries, rows])
  const retainFailed = (latest: ResolvedSelectedPlaylist, failedTrackIds: number[]) => {
    const failed = new Set(failedTrackIds)
    setSelection({ key: playlistMembershipKey(latest), rows: Object.fromEntries(
      playlistEntries(latest).filter(entry => failed.has(entry.track.id)).map(entry => [String(entry.sourceIndex), true]),
    ) })
  }
  return {
    mode, rows, setRows, selectedEntries,
    positions: selectedEntries.map(entry => entry.sourceIndex),
    trackIds: selectedEntries.map(entry => entry.track.id),
    enter: (position: number) => setRows({ [String(position)]: true }),
    exit: () => setSelection(null),
    clear: () => setRows({}),
    retainFailed,
  }
}
