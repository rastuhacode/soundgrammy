import { Music } from 'lucide-react'
import { useIntersection } from '@mantine/hooks'
import { useCachedThumbnail } from '@/hooks/use-cached-thumbnail'
import { useCacheStore } from '@/stores/cache-store'
import { cn } from '@/lib/utils'

/** Matches `size-12` thumbnail. */
const THUMB_SIZE = 48
/** Visual weight close to the old 1px CSS border, readable as progress. */
const STROKE = 1
const INSET = STROKE / 2
/**
 * `rounded-sm` = `--radius-sm` = `calc(0.75rem - 4px)` = 8px on the outer box.
 * Path is inset by half the stroke, so rx is outer radius minus inset.
 */
const CORNER = 8 - INSET

export function TrackThumbnail(props: {
  trackId: number
  fileUniqueId: string
}) {
  const { ref, entry } = useIntersection<HTMLDivElement>({ rootMargin: '400px' })
  const inView = entry?.isIntersecting ?? false
  const { url, loaded, failed } = useCachedThumbnail(props.trackId, { enabled: inView })
  const isCached = useCacheStore(state => state.cachedIds.has(props.trackId))
  const progress = useCacheStore(state => state.progressById.get(props.trackId))
  // Only real per-track download progress drives the ring. `busyIds` alone
  // (e.g. playlist cache/download marking every track) must not override the
  // cached / not-cached full border.
  const showProgress = progress !== undefined
  const progressPct = showProgress ? progress * 100 : 0

  return (
    <div
      ref={ref}
      className="relative size-12 shrink-0 overflow-hidden rounded-sm bg-muted"
      aria-label={
        showProgress
          ? 'Downloading'
          : isCached
            ? 'Cached'
            : 'Not cached'
      }
    >
      {!failed && url
        ? (
            <img
              src={url}
              alt="Thumbnail"
              decoding="async"
              className={cn(
                'absolute inset-0 size-full object-cover transition-opacity duration-200',
                loaded ? 'opacity-100' : 'opacity-0',
              )}
            />
          )
        : (
            <div className="absolute inset-0 flex items-center justify-center text-muted-foreground">
              <Music className="size-5" />
            </div>
          )}

      <svg
        className={cn(
          'pointer-events-none absolute inset-0 size-full',
          showProgress || isCached
            ? 'text-primary'
            : 'text-muted-foreground/35',
        )}
        viewBox={`0 0 ${THUMB_SIZE} ${THUMB_SIZE}`}
        aria-hidden
      >
        <rect
          x={INSET}
          y={INSET}
          width={THUMB_SIZE - STROKE}
          height={THUMB_SIZE - STROKE}
          rx={CORNER}
          ry={CORNER}
          fill="none"
          stroke="currentColor"
          strokeOpacity={showProgress ? 0.25 : 1}
          strokeWidth={STROKE}
        />
        {showProgress && (
          <rect
            x={INSET}
            y={INSET}
            width={THUMB_SIZE - STROKE}
            height={THUMB_SIZE - STROKE}
            rx={CORNER}
            ry={CORNER}
            fill="none"
            stroke="currentColor"
            strokeWidth={STROKE}
            pathLength={100}
            strokeDasharray={`${progressPct} 100`}
            strokeLinecap="butt"
          />
        )}
      </svg>
    </div>
  )
}
