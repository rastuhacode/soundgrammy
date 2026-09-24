import type { Track } from '@/lib/db'
import { Maximize2, Music } from 'lucide-react'
import { useCachedThumbnail } from '@/hooks/use-cached-thumbnail'
import { useFullscreenStore } from '@/stores/fullscreen-store'

export interface AudioTrackDescriptionProps {
  track: Track
}

export function AudioTrackDescription(props: AudioTrackDescriptionProps) {
  const { url, failed, onError } = useCachedThumbnail(props.track.id)
  const enterFullscreen = useFullscreenStore(state => state.enterFullscreen)
  const isTransitioning = useFullscreenStore(state => state.isTransitioning)

  return (
    <>
      <div className="group/thumbnail relative shrink-0">
        {failed || !url
          ? (
              <div className="flex size-11 items-center justify-center rounded-lg bg-muted text-muted-foreground md:size-16">
                <Music className="size-5" />
              </div>
            )
          : (
              <img
                src={url}
                alt="Thumbnail"
                onError={onError}
                className="size-11 rounded-lg object-cover ring-1 ring-border md:size-16"
              />
            )}
        <button
          type="button"
          onClick={enterFullscreen}
          disabled={isTransitioning}
          aria-label="Open fullscreen player"
          className="touch-visible-fullscreen absolute inset-0 hidden items-center justify-center rounded-lg bg-black/45 text-white opacity-0 backdrop-blur-[2px] transition-opacity hover:opacity-100 focus-visible:opacity-100 disabled:pointer-events-none md:flex"
        >
          <Maximize2 className="size-5 drop-shadow-md" />
        </button>
      </div>
      <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
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
