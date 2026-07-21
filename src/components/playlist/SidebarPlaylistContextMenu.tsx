import {
  EyeOff,
  Pencil,
  Trash2,
} from 'lucide-react'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'

export interface SidebarPlaylistContextMenuProps {
  children: React.ReactNode
  canEdit: boolean
  canDelete: boolean
  canHide: boolean
  isDeleting?: boolean
  onEdit?: () => void
  onDelete?: () => void
  onHide?: () => void
}

export function SidebarPlaylistContextMenu({
  children,
  canEdit,
  canDelete,
  canHide,
  isDeleting,
  onEdit,
  onDelete,
  onHide,
}: SidebarPlaylistContextMenuProps) {
  const hasActions = canEdit || canDelete || canHide
  if (!hasActions) {
    return children
  }

  return (
    <ContextMenu>
      <ContextMenuTrigger className="contents">
        {children}
      </ContextMenuTrigger>
      <ContextMenuContent className="w-44">
        {(canEdit || canDelete) && (
          <ContextMenuGroup>
            <ContextMenuLabel>Playlist</ContextMenuLabel>
            {canEdit && onEdit
              ? (
                  <ContextMenuItem onClick={onEdit}>
                    <Pencil className="size-4" />
                    Edit playlist
                  </ContextMenuItem>
                )
              : null}
            {canDelete && onDelete
              ? (
                  <ContextMenuItem
                    variant="destructive"
                    disabled={isDeleting}
                    onClick={onDelete}
                  >
                    <Trash2 className="size-4" />
                    Delete playlist
                  </ContextMenuItem>
                )
              : null}
          </ContextMenuGroup>
        )}
        {canHide && onHide
          ? (
              <>
                {(canEdit || canDelete) && <ContextMenuSeparator />}
                <ContextMenuGroup>
                  <ContextMenuLabel>Visibility</ContextMenuLabel>
                  <ContextMenuItem onClick={onHide}>
                    <EyeOff className="size-4" />
                    Hide playlist
                  </ContextMenuItem>
                </ContextMenuGroup>
              </>
            )
          : null}
      </ContextMenuContent>
    </ContextMenu>
  )
}

/** Open the nearest context-menu trigger at the click point (ellipsis → same menu as right-click). */
export function openContextMenuFromPointerEvent(
  event: React.MouseEvent,
  target: EventTarget | null,
) {
  event.preventDefault()
  event.stopPropagation()
  const el = target instanceof Element ? target : null
  const trigger = el?.closest('[data-slot="context-menu-trigger"]')
  if (!trigger) return
  trigger.dispatchEvent(
    new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: event.clientX,
      clientY: event.clientY,
      view: window,
    }),
  )
}
