import { Ellipsis } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { TrackMenuContent, type TrackMenuProps } from './TrackMenuContent'

export function PlaylistTrackDropdownMenu(props: TrackMenuProps) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={(
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={`${props.track.title ?? 'Track'} options`}
          className="touch-visible-option size-9 text-muted-foreground"
          onClick={event => event.stopPropagation()}
          onPointerDown={event => event.stopPropagation()}
        >
          <Ellipsis className="size-4" />
        </Button>
      )}
      />
      <DropdownMenuContent className="w-52" align="end"><TrackMenuContent {...props} variant="dropdown" /></DropdownMenuContent>
    </DropdownMenu>
  )
}
