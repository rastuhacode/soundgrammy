import { Ellipsis, EyeOff, FileDown, Pencil, Trash2 } from 'lucide-react'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { useRef } from 'react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useTouchScreen } from '@/hooks/use-touch-screen'
import { SidebarPlaylistContextMenu } from '@/components/playlist/SidebarPlaylistContextMenu'
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
  const touchScreen = useTouchScreen()
  const lastTouchAt = useRef<number | null>(null)
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
      disabled={touchScreen}
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
          'group flex w-full items-center gap-3 rounded-lg px-2 py-2.5 transition-colors md:py-2',
          isActive
            ? 'bg-accent text-accent-foreground'
            : 'text-foreground hover:bg-muted/70',
          isDragging && 'z-10 bg-muted opacity-90 shadow-md',
        )}
        role="button"
        aria-label={`Select ${name} playlist`}
        tabIndex={0}
        onClick={onSelect}
        onPointerDown={(event) => {
          lastTouchAt.current = event.pointerType === 'touch' ? Date.now() : null
        }}
        onContextMenuCapture={(event) => {
          if (!touchScreen && (lastTouchAt.current === null || Date.now() - lastTouchAt.current > 1000)) return
          event.preventDefault()
          event.stopPropagation()
        }}
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

          <div className="flex size-9 shrink-0 items-center justify-center md:size-6">
            {hasMenu
              ? (
                  <DropdownMenu>
                    <DropdownMenuTrigger render={(
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-xs"
                        aria-label={`${name} options`}
                        className="touch-visible-option size-9 text-muted-foreground opacity-100 transition-opacity md:size-6 md:opacity-0 md:group-hover:opacity-100 md:focus-visible:opacity-100"
                        onClick={event => event.stopPropagation()}
                        onPointerDown={event => event.stopPropagation()}
                      >
                        <Ellipsis className="size-4" />
                      </Button>
                    )}
                    />
                    <DropdownMenuContent className="w-44" align="end">
                      {canEdit && onEdit && (
                        <DropdownMenuItem onClick={onEdit}>
                          <Pencil className="size-4" />
                          Edit playlist
                        </DropdownMenuItem>
                      )}
                      {canExport && onExport && (
                        <DropdownMenuItem onClick={onExport}>
                          <FileDown className="size-4" />
                          Export playlist
                        </DropdownMenuItem>
                      )}
                      {canDelete && onDelete && (
                        <DropdownMenuItem variant="destructive" disabled={isDeleting} onClick={onDelete}>
                          <Trash2 className="size-4" />
                          Delete playlist
                        </DropdownMenuItem>
                      )}
                      {canHide && onHide && (
                        <DropdownMenuItem onClick={onHide}>
                          <EyeOff className="size-4" />
                          Hide playlist
                        </DropdownMenuItem>
                      )}
                    </DropdownMenuContent>
                  </DropdownMenu>
                )
              : null}
          </div>
        </div>
      </div>
    </SidebarPlaylistContextMenu>
  )
}
