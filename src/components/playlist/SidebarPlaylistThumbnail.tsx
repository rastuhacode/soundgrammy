import { Clock, Heart, ListMusic, Music, TrendingUp } from 'lucide-react'
import { useCachedThumbnail } from '@/hooks/use-cached-thumbnail'
import {
  ALL_TRACKS_PLAYLIST_ID,
  LIKED_PLAYLIST_ID,
  POPULAR_PLAYLIST_ID,
  RECENT_PLAYLIST_ID,
} from '@/stores/playlists-store'
import { cn } from '@/lib/utils'

export type SidebarPlaylistThumbnailVariant
  = | typeof ALL_TRACKS_PLAYLIST_ID
    | typeof LIKED_PLAYLIST_ID
    | typeof POPULAR_PLAYLIST_ID
    | typeof RECENT_PLAYLIST_ID
    | 'custom'

interface SidebarPlaylistThumbnailProps {
  variant: SidebarPlaylistThumbnailVariant
  trackIds?: number[]
  name: string
}

const THUMBNAIL_SIZE = 'size-5'

const variantStyles: Record<
  SidebarPlaylistThumbnailVariant,
  { className: string, icon: React.ReactNode }
> = {
  [ALL_TRACKS_PLAYLIST_ID]: {
    className:
      'bg-gradient-to-br from-sky-500 via-blue-600 to-indigo-700 text-white',
    icon: <ListMusic className={cn('', THUMBNAIL_SIZE)} strokeWidth={2.25} />,
  },
  [LIKED_PLAYLIST_ID]: {
    className:
      'bg-gradient-to-br from-fuchsia-500 via-purple-600 to-indigo-700 text-white',
    icon: <Heart className={cn('fill-current', THUMBNAIL_SIZE)} strokeWidth={0} />,
  },
  [POPULAR_PLAYLIST_ID]: {
    className:
      'bg-gradient-to-br from-amber-500 via-orange-600 to-rose-700 text-white',
    icon: <TrendingUp className={cn('', THUMBNAIL_SIZE)} strokeWidth={2.25} />,
  },
  [RECENT_PLAYLIST_ID]: {
    className:
      'bg-gradient-to-br from-emerald-500 via-teal-600 to-cyan-700 text-white',
    icon: <Clock className={cn('', THUMBNAIL_SIZE)} strokeWidth={2.25} />,
  },
  custom: {
    className:
      'bg-gradient-to-br from-slate-500 via-slate-600 to-slate-800 text-white/90',
    icon: <Music className={cn('', THUMBNAIL_SIZE)} strokeWidth={2.25} />,
  },
}

export function SidebarPlaylistThumbnail({
  variant,
  trackIds = [],
  name,
}: SidebarPlaylistThumbnailProps) {
  const style = variantStyles[variant]
  const coverTrackIds = variant === 'custom' ? trackIds.slice(0, 4) : []

  return (
    <div
      role="img"
      aria-label={`${name} cover`}
      className={cn(
        'relative size-9 shrink-0 overflow-hidden rounded-sm shadow-sm',
        coverTrackIds.length === 0 && style.className,
      )}
    >
      {coverTrackIds.length === 0
        ? (
            <div className="flex size-full items-center justify-center">
              {style.icon}
            </div>
          )
        : (
            <div
              className={cn(
                'grid size-full gap-px bg-border',
                coverTrackIds.length === 1 && 'grid-cols-1',
                coverTrackIds.length >= 2 && 'grid-cols-2',
                coverTrackIds.length >= 3 && 'grid-rows-2',
              )}
            >
              {coverTrackIds.map((trackId, index) => (
                <PlaylistCoverTile
                  key={`${trackId}:${index}`}
                  trackId={trackId}
                  className={coverTrackIds.length === 3 && index === 0
                    ? 'row-span-2'
                    : undefined}
                />
              ))}
            </div>
          )}
    </div>
  )
}

function PlaylistCoverTile({
  trackId,
  className,
}: {
  trackId: number
  className?: string
}) {
  const thumbnail = useCachedThumbnail(trackId)

  return (
    <div
      className={cn(
        'relative min-h-0 min-w-0 overflow-hidden bg-linear-to-br from-slate-500 to-slate-800',
        className,
      )}
    >
      {thumbnail.url
        ? (
            <img
              src={thumbnail.url}
              alt=""
              className="size-full object-cover"
            />
          )
        : (
            <div className="flex size-full items-center justify-center text-white/70">
              <Music className="size-3" strokeWidth={2.25} />
            </div>
          )}
    </div>
  )
}
