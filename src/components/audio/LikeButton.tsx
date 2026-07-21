import type { MouseEvent } from 'react'
import { Heart } from 'lucide-react'
import { fireSmallConfettiAt } from '@/lib/confetti'
import { api } from '@/lib/api'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { usePlayerStore } from '@/stores/player-store'
import { usePlaylistsStore } from '@/stores/playlists-store'

export interface LikeButtonProps {
  className?: string
}

export function LikeButton(props: LikeButtonProps) {
  const track = usePlayerStore(state => state.currentTrack)
  const playlistsData = usePlaylistsStore(state => state.data)
  const setPlaylistsData = usePlaylistsStore(state => state.setData)
  const isLiked = playlistsData?.liked.trackIds.includes(track?.id ?? 0) ?? false

  async function handleToggleLike(event: MouseEvent<HTMLButtonElement>) {
    if (!track || !playlistsData) return
    const button = event.currentTarget
    const wasLiked = isLiked
    try {
      const liked = await api.toggleLike(track.id)
      setPlaylistsData({
        ...playlistsData,
        liked,
      })
      if (!wasLiked) {
        fireSmallConfettiAt(button)
      }
    }
    catch {
      // keep UI unchanged on failure
    }
  }

  return (
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={isLiked ? 'Remove from liked' : 'Add to liked'}
      onClick={handleToggleLike}
      className={props.className}
    >
      <Heart className={cn('size-5', isLiked && 'fill-current')} />
    </Button>
  )
}
