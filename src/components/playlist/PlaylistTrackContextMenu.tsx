import type { ReactNode } from 'react'
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from '@/components/ui/context-menu'
import { TrackMenuContent, type TrackMenuProps } from './TrackMenuContent'

export interface PlaylistTrackContextMenuProps extends TrackMenuProps {
  children: ReactNode
  disabled?: boolean
}
export function PlaylistTrackContextMenu({ children, disabled, ...props }: PlaylistTrackContextMenuProps) {
  return (
    <ContextMenu disabled={disabled}>
      <ContextMenuTrigger className="contents">{children}</ContextMenuTrigger>
      <ContextMenuContent className="w-52"><TrackMenuContent {...props} variant="context" /></ContextMenuContent>
    </ContextMenu>
  )
}
