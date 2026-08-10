import { Ellipsis } from 'lucide-react'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { Button } from '@/components/ui/button'
import {
  openContextMenuFromPointerEvent,
  SidebarPlaylistContextMenu,
} from '@/components/playlist/SidebarPlaylistContextMenu'
import {
  SidebarPlaylistThumbnail,
  type SidebarPlaylistThumbnailVariant,
} from '@/components/playlist/SidebarPlaylistThumbnail'
import type { PlaylistId } from '@/stores/playlists-store'
import { cn } from '@/lib/utils'

export interface SidebarPlaylistItemProps {
  id: PlaylistId
  name: string
  count: number
  isActive: boolean
  thumbnailVariant: SidebarPlaylistThumbnailVariant
  trackIds?: number[]
  onSelect: () => void
  onEdit?: () => void
  onDelete?: () => void
  onHide?: () => void
  onExport?: () => void
  isDeleting?: boolean
  sortable?: boolean
}

export function SidebarPlaylistItem({
  id,
  name,
  count,
  isActive,
  thumbnailVariant,
  trackIds,
  onSelect,
  onEdit,
  onDelete,
  onHide,
  onExport,
  isDeleting,
  sortable = false,
}: SidebarPlaylistItemProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id,
    disabled: !sortable,
  })

  const canEdit = Boolean(onEdit)
  const canDelete = Boolean(onDelete)
  const canHide = Boolean(onHide)
  const canExport = Boolean(onExport)
  const hasMenu = canEdit || canDelete || canHide || canExport

  return (
    <SidebarPlaylistContextMenu
      canEdit={canEdit}
      canDelete={canDelete}
      canHide={canHide}
      canExport={canExport}
      isDeleting={isDeleting}
      onEdit={onEdit}
      onDelete={onDelete}
      onHide={onHide}
      onExport={onExport}
    >
      <div
        ref={setNodeRef}
        style={{
          transform: CSS.Transform.toString(transform),
          transition,
        }}
        className={cn(
          'group flex w-full items-center gap-3 rounded-lg px-2 py-2 transition-colors',
          isActive
            ? 'bg-accent text-accent-foreground'
            : 'text-foreground hover:bg-muted/70',
          isDragging && 'z-10 bg-muted opacity-90 shadow-md',
        )}
        role="button"
        aria-label={`Select ${name} playlist`}
        tabIndex={0}
        onClick={onSelect}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            onSelect()
          }
        }}
        {...(sortable ? { ...attributes, ...listeners } : {})}
      >
        <SidebarPlaylistThumbnail
          variant={thumbnailVariant}
          trackIds={trackIds}
          name={name}
        />

        <span
          className={cn(
            'min-w-0 grow truncate text-sm font-medium',
            isActive ? 'text-foreground' : 'text-foreground/90',
          )}
          title={name}
        >
          {name}
        </span>

        <div className="flex shrink-0 items-center gap-1">
          <span className="min-w-6 text-right font-mono text-xs tabular-nums text-muted-foreground">
            {count}
          </span>

          <div className="flex size-6 shrink-0 items-center justify-center">
            {hasMenu
              ? (
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`${name} options`}
                    className="text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
                    onClick={(e) => {
                      openContextMenuFromPointerEvent(e, e.currentTarget)
                    }}
                    onPointerDown={e => e.stopPropagation()}
                  >
                    <Ellipsis className="size-4" />
                  </Button>
                )
              : null}
          </div>
        </div>
      </div>
    </SidebarPlaylistContextMenu>
  )
}
