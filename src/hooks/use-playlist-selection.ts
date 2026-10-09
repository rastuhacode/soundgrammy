import { useMemo, useState } from 'react'
import type { OnChangeFn, RowSelectionState } from '@tanstack/react-table'
import { playlistEntries, playlistMembershipKey, type PlaylistEntry, type ResolvedSelectedPlaylist } from '@/lib/playlists'

/** Selection belongs to one exact ordered membership snapshot. */
export function usePlaylistSelection(playlist: ResolvedSelectedPlaylist, entries: PlaylistEntry[]) {
  const key = useMemo(() => playlistMembershipKey(playlist), [playlist])
  const [selection, setSelection] = useState<{ key: string, rows: RowSelectionState, anchor: number | null } | null>(null)
  const mode = selection?.key === key
  const rows = useMemo<RowSelectionState>(() => mode ? selection!.rows : {}, [selection, mode])
  if (selection && !mode) setSelection(null)
  const setRows: OnChangeFn<RowSelectionState> = (update) => {
    setSelection((previous) => {
      const current = previous?.key === key ? previous : null
      const nextRows = typeof update === 'function' ? update(current?.rows ?? {}) : update
      const anchor = current?.anchor != null && nextRows[String(current.anchor)] ? current.anchor : null
      return { key, rows: nextRows, anchor }
    })
  }
  const selectedEntries = useMemo(() => entries.filter(entry => rows[String(entry.sourceIndex)]), [entries, rows])
  const retainFailed = (latest: ResolvedSelectedPlaylist, failedTrackIds: number[]) => {
    const failed = new Set(failedTrackIds)
    setSelection({ key: playlistMembershipKey(latest), anchor: null, rows: Object.fromEntries(
      playlistEntries(latest).filter(entry => failed.has(entry.track.id)).map(entry => [String(entry.sourceIndex), true]),
    ) })
  }
  return {
    mode, rows, setRows, selectedEntries,
    select: (position: number, orderedEntries: PlaylistEntry[], extend: boolean, selected: boolean) => {
      setSelection((previous) => {
        const current = previous?.key === key ? previous : null
        const nextRows = { ...current?.rows }
        const anchorIndex = orderedEntries.findIndex(entry => entry.sourceIndex === current?.anchor)
        const targetIndex = orderedEntries.findIndex(entry => entry.sourceIndex === position)
        // Use the complete displayed order, including rows outside the virtual viewport.
        if (extend && anchorIndex >= 0 && targetIndex >= 0) {
          for (const entry of orderedEntries.slice(Math.min(anchorIndex, targetIndex), Math.max(anchorIndex, targetIndex) + 1)) {
            nextRows[String(entry.sourceIndex)] = true
          }
        }
        else if (extend || selected) nextRows[String(position)] = true
        else delete nextRows[String(position)]
        return {
          key, rows: nextRows,
          anchor: extend || selected ? position : current?.anchor === position ? null : current?.anchor ?? null,
        }
      })
    },
    positions: selectedEntries.map(entry => entry.sourceIndex),
    trackIds: selectedEntries.map(entry => entry.track.id),
    enter: (position: number) => setSelection({ key, rows: { [String(position)]: true }, anchor: position }),
    exit: () => setSelection(null),
    clear: () => setRows({}),
    retainFailed,
  }
}
