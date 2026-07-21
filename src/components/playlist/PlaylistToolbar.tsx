import { Play, Search, Shuffle, Undo2, X } from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import { Separator } from '@/components/ui/separator'
import { Button } from '@/components/ui/button'
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from '@/components/ui/input-group'
import type { ResolvedSelectedPlaylist } from '@/stores/playlists-store'
import { PlaylistBulkActions } from './PlaylistBulkActions'
import type { CustomPlaylistRef } from './track-actions'

export interface PlaylistToolbarProps {
  search: string
  onSearchChange: (value: string) => void
  selectionMode: boolean
  selectedTrackIds: number[]
  selectedPositions: number[]
  currentPlaylist: ResolvedSelectedPlaylist
  customPlaylists: CustomPlaylistRef[]
  likedTrackIds: Set<number>
  onPlay: () => void
  onShuffle: () => void
  onExitSelection: () => void
  onAddToLiked: (trackIds: number[]) => void
  onRemoveFromLiked: (trackIds: number[]) => void
  onAddToPlaylist: (playlistId: number, trackIds: number[]) => void
  onRemoveFromPlaylist: (playlistId: number, positions: number[]) => void
  onPlayNext: () => void
  onAddToEnd: () => void
  onDownload: (trackIds: number[]) => void
}

export function PlaylistToolbar({
  search,
  onSearchChange,
  selectionMode,
  selectedTrackIds,
  selectedPositions,
  currentPlaylist,
  customPlaylists,
  likedTrackIds,
  onPlay,
  onShuffle,
  onExitSelection,
  onAddToLiked,
  onRemoveFromLiked,
  onAddToPlaylist,
  onRemoveFromPlaylist,
  onPlayNext,
  onAddToEnd,
  onDownload,
}: PlaylistToolbarProps) {
  return (
    <div className="flex h-fit w-full shrink-0 items-center justify-between gap-4 px-4">
      <div className="grow flex gap-2">
        <Button size="icon" onClick={onPlay}>
          <Play className="size-4" />
        </Button>
        <Button variant="secondary" onClick={onShuffle}>
          <Shuffle className="size-4" />
          Shuffle
        </Button>

        <AnimatePresence initial={false}>
          {selectionMode && (
            <motion.div
              key="bulk-actions"
              initial={{ opacity: 0, x: -8 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -8 }}
              transition={{ duration: 0.18, ease: 'easeOut' }}
              className="flex items-center gap-2"
            >
              <Separator orientation="vertical" className="h-full" />

              {selectedTrackIds.length > 0 && (
                <PlaylistBulkActions
                  selectedTrackIds={selectedTrackIds}
                  selectedPositions={selectedPositions}
                  currentPlaylist={currentPlaylist}
                  customPlaylists={customPlaylists}
                  likedTrackIds={likedTrackIds}
                  onAddToLiked={onAddToLiked}
                  onRemoveFromLiked={onRemoveFromLiked}
                  onAddToPlaylist={onAddToPlaylist}
                  onRemoveFromPlaylist={onRemoveFromPlaylist}
                  onPlayNext={onPlayNext}
                  onAddToEnd={onAddToEnd}
                  onDownload={onDownload}
                />
              )}

              <Button variant="outline" size="icon" onClick={onExitSelection}>
                <Undo2 className="size-4" />
              </Button>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <div className="min-w-40 max-w-xl w-full px-2">
        <InputGroup>
          <InputGroupInput
            value={search}
            onChange={e => onSearchChange(e.target.value)}
            placeholder="Search tracks"
          />
          <InputGroupAddon>
            <Search className="size-4" />
          </InputGroupAddon>
          {search.length > 0 && (
            <InputGroupButton onClick={() => onSearchChange('')}>
              <X className="size-4" />
            </InputGroupButton>
          )}
        </InputGroup>
      </div>
    </div>
  )
}
