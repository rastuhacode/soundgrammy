import type { Track } from '@/lib/db'
import { Maximize2, Music } from 'lucide-react'
import { useCachedThumbnail } from '@/hooks/use-cached-thumbnail'
import { useFullscreenStore } from '@/stores/fullscreen-store'

export interface AudioTrackDescriptionProps {
  track: Track
}

export function AudioTrackDescription(props: AudioTrackDescriptionProps) {
  const { url, failed } = useCachedThumbnail(props.track.id)
  const enterFullscreen = useFullscreenStore(state => state.enterFullscreen)
  const isTransitioning = useFullscreenStore(state => state.isTransitioning)

  return (
    <>
      <div className="group/thumbnail relative shrink-0">
        {failed || !url
          ? (
              <div className="flex size-16 items-center justify-center rounded-lg bg-muted text-muted-foreground">
                <Music className="size-5" />
              </div>
            )
          : (
              <img
                src={url}
                alt="Thumbnail"
                className="size-16 rounded-lg object-cover ring-1 ring-border"
              />
            )}
        <button
          type="button"
          onClick={enterFullscreen}
          disabled={isTransitioning}
          aria-label="Open fullscreen player"
          className="absolute inset-0 flex items-center justify-center rounded-lg bg-black/45 text-white opacity-0 backdrop-blur-[2px] transition-opacity group-hover/thumbnail:opacity-100 focus-visible:opacity-100 disabled:pointer-events-none"
        >
          <Maximize2 className="size-5 drop-shadow-md" />
        </button>
      </div>
      <div className="hidden min-w-0 flex-col sm:flex">
        <span
          className="truncate text-sm font-medium text-foreground"
          title={props.track.title ?? 'Unknown Title'}
        >
          {props.track.title ?? 'Unknown Title'}
        </span>
        <span
          className="truncate text-xs text-muted-foreground"
          title={props.track.performer ?? 'Unknown Artist'}
        >
          {props.track.performer ?? 'Unknown Artist'}
        </span>
      </div>
    </>
  )
}
