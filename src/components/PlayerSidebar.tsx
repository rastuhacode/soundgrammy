import { Plus } from 'lucide-react'
import {
  DndContext,
  closestCenter,
  type Modifier,
} from '@dnd-kit/core'
import {
  SortableContext,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { PlaylistFormDialog } from '@/components/playlist/PlaylistFormDialog'
import { SidebarPlaylistItem } from '@/components/playlist/SidebarPlaylistItem'
import { SidebarPlaylistsToolbar } from '@/components/playlist/SidebarPlaylistsToolbar'
import { canHidePlaylist } from '@/lib/playlist-visibility'
import { useSidebarPlaylists } from '@/hooks/use-sidebar-playlists'
import {
  exportPlaylistRecipeFile,
  formatInvokeError,
} from '@/lib/playlist-recipe-io'
import type { PlaylistRecipeSource } from '@/types'
import { canExportPlaylist } from '@/components/playlist/track-actions'
import { SidebarProfile } from './SidebarProfile'

const restrictToVerticalAxis: Modifier = ({ transform }) => ({
  ...transform,
  x: 0,
})

export function PlayerSidebar(props: { onLogout: () => void }) {
  const {
    selectedPlaylistId,
    setSelectedPlaylist,
    dialogState,
    setDialogState,
    deletingId,
    search,
    setSearch,
    sortMode,
    sortReversed,
    hiddenEntries,
    filteredPlaylists,
    canReorder,
    sensors,
    handleSortModeChange,
    handleSortReversedChange,
    handleDragEnd,
    handleHide,
    handleUnhide,
    handleDelete,
  } = useSidebarPlaylists()

  const [actionError, setActionError] = useState<string | null>(null)

  const title = 'Create playlist'

  const handleExportPlaylist = async (
    source: PlaylistRecipeSource,
    name: string,
  ) => {
    try {
      await exportPlaylistRecipeFile({ source, name })
    }
    catch (error) {
      setActionError(formatInvokeError(error))
    }
  }

  return (
    <div className="flex h-full grow flex-col gap-4 pt-4">
      <div className="flex items-center justify-between gap-2 px-4">
        <div className="flex items-center gap-4">
          <SidebarProfile onLogout={props.onLogout} />
          <h2 className="text-lg font-semibold tracking-tight text-foreground">
            Library
          </h2>
        </div>
        <div className="flex items-center gap-0.5">
          <SidebarPlaylistsToolbar
            search={search}
            onSearchChange={setSearch}
            sortMode={sortMode}
            onSortModeChange={handleSortModeChange}
            sortReversed={sortReversed}
            onSortReversedChange={handleSortReversedChange}
            hiddenEntries={hiddenEntries}
            onUnhide={handleUnhide}
          />
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => setDialogState({ mode: 'create' })}
            aria-label={title}
            title={title}
            className="text-muted-foreground hover:text-foreground"
          >
            <Plus />
          </Button>
        </div>
      </div>

      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        modifiers={[restrictToVerticalAxis]}
        onDragEnd={handleDragEnd}
      >
        <SortableContext
          items={filteredPlaylists.map(item => item.id)}
          strategy={verticalListSortingStrategy}
        >
          <div className="flex min-h-0 grow flex-col gap-0.5 overflow-x-hidden overflow-y-auto px-2 pb-2">
            {filteredPlaylists.map((item) => {
              const customPlaylist = item.playlist
              const hideId = canHidePlaylist(item.id) ? item.id : null
              const exportable = canExportPlaylist({
                id: item.id,
                isCustom: Boolean(customPlaylist),
              })

              return (
                <SidebarPlaylistItem
                  key={String(item.id)}
                  id={item.id}
                  name={item.name}
                  count={item.count}
                  isActive={selectedPlaylistId === item.id}
                  thumbnailVariant={item.thumbnailVariant}
                  trackIds={item.trackIds}
                  sortable={canReorder}
                  onSelect={() => setSelectedPlaylist(item.id)}
                  onEdit={customPlaylist
                    ? () => setDialogState({ mode: 'edit', playlist: customPlaylist })
                    : undefined}
                  onDelete={customPlaylist
                    ? () => handleDelete(customPlaylist.id)
                    : undefined}
                  onHide={hideId ? () => handleHide(hideId) : undefined}
                  onExport={exportable
                    ? () => {
                        const source: PlaylistRecipeSource = customPlaylist
                          ? { kind: 'custom', playlistId: customPlaylist.id }
                          : { kind: 'liked' }
                        handleExportPlaylist(source, item.name)
                      }
                    : undefined}
                  isDeleting={customPlaylist ? deletingId === customPlaylist.id : undefined}
                />
              )
            })}
          </div>
        </SortableContext>
      </DndContext>

      <PlaylistFormDialog
        open={dialogState !== null}
        onOpenChange={(open) => {
          if (!open) setDialogState(null)
        }}
        mode={dialogState?.mode ?? 'create'}
        playlist={dialogState?.mode === 'edit' ? dialogState.playlist : undefined}
      />

      <Dialog
        open={actionError !== null}
        onOpenChange={(open) => {
          if (!open) setActionError(null)
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Couldn’t finish</DialogTitle>
            <DialogDescription className="whitespace-pre-wrap">
              {actionError}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="secondary" onClick={() => setActionError(null)}>
              OK
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
