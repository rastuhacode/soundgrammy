import { useIntersection } from '@mantine/hooks'
import { Music } from 'lucide-react'
import { useCachedThumbnail } from '@/hooks/use-cached-thumbnail'

export function TrackThumbnail(props: {
  trackId: number
  fileUniqueId: string
}) {
  const { ref, entry } = useIntersection<HTMLDivElement>({ rootMargin: '400px' })
  const inView = entry?.isIntersecting ?? false
  const { url, loaded, failed } = useCachedThumbnail(props.trackId, { enabled: inView })

  return (
    <div
      ref={ref}
      className="relative size-12 shrink-0 overflow-hidden rounded-sm bg-muted"
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
