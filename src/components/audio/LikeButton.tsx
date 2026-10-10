import { Heart } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { useState } from 'react'
import { errorMessage } from '@/lib/errors'
import { usePlayerStore } from '@/stores/player-store'
import { usePlaylistsStore } from '@/stores/playlists-store'

export interface LikeButtonProps {
  className?: string
}

export function LikeButton(props: LikeButtonProps) {
  const track = usePlayerStore(state => state.currentTrack)
  const playlistsData = usePlaylistsStore(state => state.data)
  const toggleLike = usePlaylistsStore(state => state.toggleLike)
  const [error, setError] = useState<string | null>(null)
  const isLiked = playlistsData?.liked.trackIds.includes(track?.id ?? 0) ?? false

  const title = isLiked ? 'Remove from liked' : 'Add to liked'

  async function handleToggleLike() {
    if (!track || !playlistsData) return
    try {
      await toggleLike(track.id)
      setError(null)
    }
    catch (error) {
      setError(errorMessage(error))
    }
  }

  return (
    <>
      {error && <span role="alert" className="text-xs text-destructive">{error}</span>}
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={title}
        title={title}
        onClick={handleToggleLike}
        className={props.className}
      >
        <Heart className={cn('size-5', isLiked && 'fill-current')} />
      </Button>
    </>
  )
}
