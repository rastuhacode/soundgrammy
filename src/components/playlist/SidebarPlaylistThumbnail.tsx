import { Clock, Heart, ListMusic, Music, TrendingUp } from 'lucide-react'
import { usePlaylistThumbnail } from '@/hooks/use-playlist-thumbnail'
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
  playlistId?: number
  hasThumbnail?: boolean
  name: string
}

const variantStyles: Record<
  SidebarPlaylistThumbnailVariant,
  { className: string, icon: React.ReactNode }
> = {
  [ALL_TRACKS_PLAYLIST_ID]: {
    className:
      'bg-gradient-to-br from-sky-500 via-blue-600 to-indigo-700 text-white',
    icon: <ListMusic className="size-4" strokeWidth={2.25} />,
  },
  [LIKED_PLAYLIST_ID]: {
    className:
      'bg-gradient-to-br from-fuchsia-500 via-purple-600 to-indigo-700 text-white',
    icon: <Heart className="size-4 fill-current" strokeWidth={0} />,
  },
  [POPULAR_PLAYLIST_ID]: {
    className:
      'bg-gradient-to-br from-amber-500 via-orange-600 to-rose-700 text-white',
    icon: <TrendingUp className="size-4" strokeWidth={2.25} />,
  },
  [RECENT_PLAYLIST_ID]: {
    className:
      'bg-gradient-to-br from-emerald-500 via-teal-600 to-cyan-700 text-white',
    icon: <Clock className="size-4" strokeWidth={2.25} />,
  },
  custom: {
    className:
      'bg-gradient-to-br from-slate-500 via-slate-600 to-slate-800 text-white/90',
    icon: <Music className="size-4" strokeWidth={2.25} />,
  },
}

export function SidebarPlaylistThumbnail({
  variant,
  playlistId,
  hasThumbnail = false,
  name,
}: SidebarPlaylistThumbnailProps) {
  const coverUrl = usePlaylistThumbnail(
    variant === 'custom' ? playlistId : undefined,
    hasThumbnail,
  )
  const showImage = variant === 'custom' && hasThumbnail && Boolean(coverUrl)
  const style = variantStyles[variant]

  return (
    <div
      className={cn(
        'relative size-12 shrink-0 overflow-hidden rounded-sm shadow-sm',
        !showImage && style.className,
      )}
    >
      {!showImage
        ? (
            <div className="flex size-full items-center justify-center">
              {style.icon}
            </div>
          )
        : (
            <img
              src={coverUrl ?? undefined}
              alt={`${name} cover`}
              className="size-full object-cover"
            />
          )}
    </div>
  )
}
