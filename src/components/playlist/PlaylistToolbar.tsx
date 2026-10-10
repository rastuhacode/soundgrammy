import type { ReactNode, Ref } from 'react'
import { ArrowLeft, Search, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useCompactDisplay } from '@/hooks/use-compact-display'
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '@/components/ui/input-group'
import type { ResolvedSelectedPlaylist } from '@/lib/playlists'

export interface PlaylistToolbarProps {
  onBack?: () => void
  currentPlaylist: ResolvedSelectedPlaylist
  search: string
  onSearchChange: (search: string) => void
  onOpenSearch: () => void
  searchButtonRef?: Ref<HTMLButtonElement>
  actions: ReactNode
}

/** Layout and search stay independent of playback and selection actions. */
export function PlaylistToolbar({ onBack, currentPlaylist, search, onSearchChange,
  onOpenSearch, searchButtonRef, actions }: PlaylistToolbarProps) {
  const { isCompact } = useCompactDisplay()
  const searchControl = isCompact
    ? (
        <Button
          ref={searchButtonRef}
          variant="outline"
          size="icon"
          onClick={onOpenSearch}
          aria-label="Search tracks"
          title={search ? `Search: ${search}` : 'Search tracks'}
          className={search ? 'ml-auto text-primary' : 'ml-auto'}
        >
          <Search className="size-5" />
        </Button>
      )
    : (
        <div className="min-w-48 flex-1">
          <InputGroup>
            <InputGroupInput
              type="search"
              aria-label="Search tracks"
              value={search}
              onChange={event => onSearchChange(event.target.value)}
              placeholder="Search tracks"
            />
            <InputGroupAddon>
              <Search className="size-4" />
            </InputGroupAddon>
            {search.length > 0 && (
              <InputGroupButton aria-label="Clear track search" onClick={() => onSearchChange('')}>
                <X className="size-4" />
              </InputGroupButton>
            )}
          </InputGroup>
        </div>
      )

  if (onBack) {
    const trackCount = currentPlaylist.trackIds.length
    return (
      <header className="flex w-full min-w-0 shrink-0 flex-nowrap items-center gap-1 border-b border-border bg-sidebar/85 px-2 pb-2 pt-[max(0.5rem,env(safe-area-inset-top))] backdrop-blur-sm">
        <Button variant="ghost" size="icon" className="size-10" onClick={onBack} aria-label="Back to playlists" title="Back to playlists"><ArrowLeft className="size-5" /></Button>
        <div className="min-w-0 flex-1 px-1">
          <h1 className="truncate text-base font-semibold" title={currentPlaylist.name}>{currentPlaylist.name}</h1>
          <p className="text-xs text-muted-foreground">
            {trackCount}
            {' '}
            {trackCount === 1 ? 'track' : 'tracks'}
          </p>
        </div>
        <div className="compact-playlist-actions flex min-w-43 grow items-center gap-1">
          {actions}
          {searchControl}
        </div>
      </header>
    )
  }
  return (
    <div className="flex h-fit w-full min-w-0 shrink-0 flex-wrap items-center gap-4 px-4">
      <div className="flex max-w-full flex-none flex-wrap items-center gap-2">{actions}</div>
      {searchControl}
    </div>
  )
}
