import { ArrowUpDown, List, Search, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from '@/components/ui/input-group'
import {
  PLAYLIST_SORT_MODE_LABELS,
  type PlaylistSortMode,
} from '@/lib/playlist-sort'
import type { HideablePlaylistId } from '@/lib/playlist-visibility'

const SORT_MODES: PlaylistSortMode[] = ['recency', 'custom', 'alphabetical']

export interface SidebarPlaylistsToolbarProps {
  search: string
  onSearchChange: (value: string) => void
  sortMode: PlaylistSortMode
  onSortModeChange: (mode: PlaylistSortMode) => void
  sortReversed: boolean
  onSortReversedChange: (reversed: boolean) => void
  hiddenEntries: Array<{ id: HideablePlaylistId, name: string }>
  onUnhide: (id: HideablePlaylistId) => void
}

export function SidebarPlaylistsToolbar({
  search,
  onSearchChange,
  sortMode,
  onSortModeChange,
  sortReversed,
  onSortReversedChange,
  hiddenEntries,
  onUnhide,
}: SidebarPlaylistsToolbarProps) {
  return (
    <div className="flex items-center gap-1 px-4 justify-between">
      <div className="w-1/2">
        <InputGroup>
          <InputGroupInput
            value={search}
            onChange={e => onSearchChange(e.target.value)}
            placeholder="Search"
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

      <DropdownMenu>
        <DropdownMenuTrigger
          render={(
            <Button
              aria-label="Sort and filter playlists"
              variant="ghost"
              className="shrink-0 text-muted-foreground"
            >
              {PLAYLIST_SORT_MODE_LABELS[sortMode]}
              <List className="size-4" />
            </Button>
          )}
        />
        <DropdownMenuContent align="end" className="w-52">
          <DropdownMenuRadioGroup
            value={sortMode}
            onValueChange={value => onSortModeChange(value as PlaylistSortMode)}
          >
            {SORT_MODES.map(mode => (
              <DropdownMenuRadioItem key={mode} value={mode}>
                {PLAYLIST_SORT_MODE_LABELS[mode]}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
          <DropdownMenuSeparator />
          <DropdownMenuCheckboxItem
            checked={sortReversed && sortMode !== 'custom'}
            disabled={sortMode === 'custom'}
            onCheckedChange={(checked) => {
              if (sortMode === 'custom') return
              onSortReversedChange(checked)
            }}
          >
            <ArrowUpDown className="size-4" />
            Reverse
          </DropdownMenuCheckboxItem>
          {hiddenEntries.length > 0
            ? (
                <>
                  <DropdownMenuSeparator />
                  <DropdownMenuGroup>
                    <DropdownMenuLabel>Hidden Playlists</DropdownMenuLabel>
                    {hiddenEntries.map(entry => (
                      <DropdownMenuItem
                        key={entry.id}
                        onClick={() => onUnhide(entry.id)}
                        className="justify-between gap-2 group/hidden-playlist"
                      >
                        {entry.name}
                        <X className="size-4 opacity-0 transition-opacity group-hover/hidden-playlist:opacity-100" />
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuGroup>
                </>
              )
            : null}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  )
}
