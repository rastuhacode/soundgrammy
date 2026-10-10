import { Undo2 } from 'lucide-react'
import { motion } from 'motion/react'
import { PlaylistBulkActions, type PlaylistBulkActionsProps } from './PlaylistBulkActions'
import { PlaylistActionButton } from './PlaylistPlaybackActions'

export interface PlaylistSelectionActionsProps extends PlaylistBulkActionsProps {
  onExitSelection: () => void
}

export function PlaylistSelectionActions({ onExitSelection, ...bulk }: PlaylistSelectionActionsProps) {
  return (
    <motion.div initial={{ opacity: 0, x: -8 }} animate={{ opacity: 1, x: 0 }} transition={{ duration: 0.18, ease: 'easeOut' }} className="flex shrink-0 items-center gap-2">
      {bulk.selectedTrackIds.length > 0 && <PlaylistBulkActions {...bulk} />}
      <PlaylistActionButton label="Exit selection" variant="outline" onClick={onExitSelection}><Undo2 className="size-4" /></PlaylistActionButton>
    </motion.div>
  )
}
