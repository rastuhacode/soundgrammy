import { Check, Funnel, FunnelX, Search, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from '@/components/ui/input-group'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Separator } from '@/components/ui/separator'
import { cn } from '@/lib/utils'
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
  const hasActiveFilters = search.length > 0
    || sortMode !== 'custom'
    || sortReversed
  const title = 'Playlists filters'

  return (
    <Popover>
      <PopoverTrigger
        render={(
          <Button
            aria-label={title}
            title={title}
            variant="ghost"
            size="icon-sm"
            className="text-muted-foreground hover:text-foreground"
          >
            { hasActiveFilters ? <FunnelX /> : <Funnel />}
          </Button>
        )}
      />
      <PopoverContent className="w-56 gap-1.5 p-2">
        <InputGroup>
          <InputGroupInput
            value={search}
            onChange={e => onSearchChange(e.target.value)}
            placeholder="Search playlists"
            autoFocus
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

        <Separator />

        <div role="radiogroup" aria-label="Sort playlists" className="flex flex-col gap-0.5">
          {SORT_MODES.map(mode => (
            <button
              key={mode}
              type="button"
              role="radio"
              aria-checked={sortMode === mode}
              onClick={() => onSortModeChange(mode)}
              className={cn(
                'relative flex w-full cursor-default items-center gap-1.5 rounded-md py-1 pr-8 pl-1.5 text-left text-sm outline-hidden select-none hover:bg-accent hover:text-accent-foreground',
                sortMode === mode && 'bg-accent text-accent-foreground',
              )}
            >
              {PLAYLIST_SORT_MODE_LABELS[mode]}
              {sortMode === mode && (
                <Check className="pointer-events-none absolute right-2 size-4" />
              )}
            </button>
          ))}
        </div>

        <Separator />

        <label
          className={cn(
            'flex cursor-pointer items-center gap-2 rounded-md px-1.5 py-1 text-sm select-none hover:bg-accent hover:text-accent-foreground',
            sortMode === 'custom' && 'pointer-events-none opacity-50',
          )}
        >
          <Checkbox
            checked={sortReversed && sortMode !== 'custom'}
            disabled={sortMode === 'custom'}
            onCheckedChange={(checked) => {
              if (sortMode === 'custom') return
              onSortReversedChange(checked === true)
            }}
          />
          Reverse
        </label>

        {hiddenEntries.length > 0
          ? (
              <>
                <Separator />
                <div className="flex flex-col gap-0.5">
                  <span className="px-1.5 py-1 text-xs font-medium text-muted-foreground">
                    Hidden Playlists
                  </span>
                  {hiddenEntries.map(entry => (
                    <button
                      key={entry.id}
                      type="button"
                      onClick={() => onUnhide(entry.id)}
                      className="group/hidden-playlist flex w-full items-center justify-between gap-2 rounded-md px-1.5 py-1 text-left text-sm outline-hidden select-none hover:bg-accent hover:text-accent-foreground"
                    >
                      {entry.name}
                      <X className="size-4 opacity-0 transition-opacity group-hover/hidden-playlist:opacity-100" />
                    </button>
                  ))}
                </div>
              </>
            )
          : null}
      </PopoverContent>
    </Popover>
  )
}
