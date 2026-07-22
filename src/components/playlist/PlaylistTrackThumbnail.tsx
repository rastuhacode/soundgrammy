import { Music } from 'lucide-react'
import { useIntersection } from '@mantine/hooks'
import { useCachedThumbnail } from '@/hooks/use-cached-thumbnail'
import { useCacheStore } from '@/stores/cache-store'
import { cn } from '@/lib/utils'

export function TrackThumbnail(props: {
  trackId: number
  fileUniqueId: string
}) {
  const { ref, entry } = useIntersection<HTMLDivElement>({ rootMargin: '400px' })
  const inView = entry?.isIntersecting ?? false
  const { url, loaded, failed } = useCachedThumbnail(props.trackId, { enabled: inView })
  const isCached = useCacheStore(state => state.cachedIds.has(props.trackId))

  return (
    <div
      ref={ref}
      className={cn(
        'relative size-12 shrink-0 overflow-hidden rounded-sm bg-muted',
        'border transition-[border-color] duration-300 ease-out',
        isCached ? 'border-primary' : 'border-muted-foreground/35',
      )}
      aria-label={isCached ? 'Cached' : 'Not cached'}
    >
      {!failed && url
        ? (
            <img
              src={url}
              alt="Thumbnail"
              decoding="async"
              className={`absolute inset-0 size-12 object-cover transition-opacity duration-200 ${
                loaded ? 'opacity-100' : 'opacity-0'
              }`}
            />
          )
        : (
            <div className="absolute inset-0 flex items-center justify-center text-muted-foreground">
              <Music className="size-5" />
            </div>
          )}
    </div>
  )
}
