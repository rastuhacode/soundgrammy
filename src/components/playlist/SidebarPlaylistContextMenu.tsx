import {
  EyeOff,
  FileDown,
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
  canExport?: boolean
  isDeleting?: boolean
  onEdit?: () => void
  onDelete?: () => void
  onHide?: () => void
  onExport?: () => void
}

export function SidebarPlaylistContextMenu({
  children,
  canEdit,
  canDelete,
  canHide,
  canExport,
  isDeleting,
  onEdit,
  onDelete,
  onHide,
  onExport,
}: SidebarPlaylistContextMenuProps) {
  const hasPlaylistActions = canEdit || canDelete || Boolean(canExport)
  const hasActions = hasPlaylistActions || canHide
  if (!hasActions) {
    return children
  }

  return (
    <ContextMenu>
      <ContextMenuTrigger className="contents">
        {children}
      </ContextMenuTrigger>
      <ContextMenuContent className="w-44">
        {hasPlaylistActions
          ? (
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
                {canExport && onExport
                  ? (
                      <ContextMenuItem onClick={onExport}>
                        <FileDown className="size-4" />
                        Export playlist
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
            )
          : null}
        {canHide && onHide
          ? (
              <>
                {hasPlaylistActions && <ContextMenuSeparator />}
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
  const targetRect = el?.getBoundingClientRect()
  const fromKeyboard = event.detail === 0
  trigger.dispatchEvent(
    new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: fromKeyboard && targetRect
        ? targetRect.left + targetRect.width / 2
        : event.clientX,
      clientY: fromKeyboard && targetRect
        ? targetRect.bottom
        : event.clientY,
      view: window,
    }),
  )
}
