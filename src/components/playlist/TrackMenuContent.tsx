import { Fragment } from 'react'
import { CheckSquare, Download, HardDriveDownload, HardDriveUpload, Heart, Info, ListEnd, ListPlus, ListStart, ListX, Plus, type LucideIcon } from 'lucide-react'
import type { Track } from '@/types'
import type { ResolvedSelectedPlaylist } from '@/lib/playlists'
import { getTrackContextActions, type CustomPlaylistRef } from '@/lib/playlist-track-actions'
import { useCacheStore } from '@/stores/cache-store'
import * as Context from '@/components/ui/context-menu'
import * as Dropdown from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'

export interface TrackMenuHandlers {
  onToggleLike: (trackId: number) => void
  onAddToPlaylist: (playlistId: number, trackId: number) => void
  onCreatePlaylist: (trackId: number) => void
  onDeleteFromPlaylist: (playlistId: number, position: number) => void
  onPlayNext: (track: Track) => void
  onAddToEnd: (track: Track) => void
  onCache: (track: Track) => void
  onDownload: (track: Track) => void
  onRemoveFromCache: (track: Track) => void
  onShowInfo: (track: Track) => void
}
export interface TrackMenuProps extends TrackMenuHandlers {
  track: Track
  sourceIndex: number
  isLiked: boolean
  currentPlaylist: ResolvedSelectedPlaylist
  customPlaylists: CustomPlaylistRef[]
  onSelect: (sourceIndex: number) => void
}
interface MenuAction {
  label: string
  icon: LucideIcon
  run: () => void
  disabled?: boolean
  active?: boolean
}

/** One action tree for mouse context menus and touch dropdowns. */
export function TrackMenuContent(props: TrackMenuProps & { variant: 'context' | 'dropdown' }) {
  const { track, sourceIndex, isLiked, currentPlaylist, customPlaylists } = props
  const capabilities = getTrackContextActions(currentPlaylist)
  const isCached = useCacheStore(state => state.cachedIds.has(track.id))
  const isBusy = useCacheStore(state => state.busyIds.has(track.id))
  const menu = props.variant === 'context'
    ? { Group: Context.ContextMenuGroup, Item: Context.ContextMenuItem, Label: Context.ContextMenuLabel,
        Separator: Context.ContextMenuSeparator, Sub: Context.ContextMenuSub, SubTrigger: Context.ContextMenuSubTrigger, SubContent: Context.ContextMenuSubContent }
    : { Group: Dropdown.DropdownMenuGroup, Item: Dropdown.DropdownMenuItem, Label: Dropdown.DropdownMenuLabel,
        Separator: Dropdown.DropdownMenuSeparator, Sub: Dropdown.DropdownMenuSub, SubTrigger: Dropdown.DropdownMenuSubTrigger, SubContent: Dropdown.DropdownMenuSubContent }
  const { Group, Item, Label, Separator, Sub, SubTrigger, SubContent } = menu
  const groups: { label: string, items: MenuAction[] }[] = [
    { label: 'Selection', items: [{ label: 'Select', icon: CheckSquare, run: () => props.onSelect(sourceIndex) }] },
    { label: 'Queue', items: [
      { label: 'Play next', icon: ListStart, run: () => props.onPlayNext(track) },
      { label: 'Add to end', icon: ListEnd, run: () => props.onAddToEnd(track) },
    ] },
    { label: 'Playlist', items: [
      { label: isLiked ? 'Remove from Liked' : 'Add to Liked', icon: Heart, active: isLiked, run: () => props.onToggleLike(track.id) },
      ...(capabilities.removeFromPlaylist && currentPlaylist.isCustom
        ? [{ label: 'Remove from playlist', icon: ListX, run: () => props.onDeleteFromPlaylist(currentPlaylist.id as number, sourceIndex) }]
        : []),
    ] },
    { label: 'Track', items: [
      isCached
        ? { label: 'Remove from cache', icon: HardDriveUpload, run: () => props.onRemoveFromCache(track) }
        : { label: isBusy ? 'Caching…' : 'Cache', icon: HardDriveDownload, disabled: isBusy, run: () => props.onCache(track) },
      { label: isBusy ? 'Downloading…' : 'Download', icon: Download, disabled: isBusy, run: () => props.onDownload(track) },
      { label: 'Show info', icon: Info, run: () => props.onShowInfo(track) },
    ] },
  ]
  const renderAction = ({ label, icon: Icon, run, disabled, active }: MenuAction) => (
    <Item key={label} onClick={run} disabled={disabled}>
      <Icon className={cn('size-4', active && 'fill-primary text-primary')} />
      {label}
    </Item>
  )
  return groups.map((group, index) => (
    <Fragment key={group.label}>
      {index > 0 && <Separator />}
      <Group>
        <Label>{group.label}</Label>
        {group.label === 'Playlist'
          ? (
              <>
                {renderAction(group.items[0]!)}
                <Sub>
                  <SubTrigger>
                    <ListPlus className="size-4" />
                    Add to playlist
                  </SubTrigger>
                  <SubContent className="max-w-[min(16rem,calc(100vw-2rem))]">
                    <Item onClick={() => props.onCreatePlaylist(track.id)}>
                      <Plus className="size-4" />
                      Create playlist
                    </Item>
                    {customPlaylists.length > 0 && <Separator />}
                    {customPlaylists.map(playlist => (
                      <Item key={playlist.id} title={playlist.name} onClick={() => props.onAddToPlaylist(playlist.id, track.id)}>
                        <ListPlus className="size-4" />
                        <span className="min-w-0 truncate">{playlist.name}</span>
                      </Item>
                    ))}
                  </SubContent>
                </Sub>
                {group.items.slice(1).map(renderAction)}
              </>
            )
          : group.items.map(renderAction)}
      </Group>
    </Fragment>
  ))
}
