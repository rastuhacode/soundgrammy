import { Music } from 'lucide-react'
import {
  LIKED_PLAYLIST_ID,
  POPULAR_PLAYLIST_ID,
  RECENT_PLAYLIST_ID,
  type ResolvedSelectedPlaylist,
} from '@/stores/playlists-store'

function getEmptyStateCopy(
  libraryTrackCount: number,
  playlistId: ResolvedSelectedPlaylist['id'],
  isCustom: boolean,
): { title: string, description: string } {
  if (libraryTrackCount === 0) {
    return {
      title: 'No tracks yet',
      description:
        'Pin music to your Telegram profile and it will tune in here automatically.',
    }
  }

  if (playlistId === LIKED_PLAYLIST_ID) {
    return {
      title: 'No liked tracks yet',
      description: 'Tap the heart on any track to save it here.',
    }
  }

  if (playlistId === POPULAR_PLAYLIST_ID || playlistId === RECENT_PLAYLIST_ID) {
    return {
      title: 'No listening history yet',
      description: 'Play some tracks and they will show up here.',
    }
  }

  if (isCustom) {
    return {
      title: 'This playlist is empty',
      description: 'Add tracks from your library using the list button.',
    }
  }

  return {
    title: 'No tracks yet',
    description:
      'Pin music to your Telegram profile and it will tune in here automatically.',
  }
}

export interface PlaylistEmptyStateProps {
  libraryTrackCount: number
  playlistId: ResolvedSelectedPlaylist['id']
  isCustom: boolean
}

export function PlaylistEmptyState({
  libraryTrackCount,
  playlistId,
  isCustom,
}: PlaylistEmptyStateProps) {
  const emptyState = getEmptyStateCopy(libraryTrackCount, playlistId, isCustom)

  return (
    <div className="animate-fade-up flex flex-col items-center justify-center gap-3 px-6 py-20 text-center">
      <div className="flex size-14 items-center justify-center rounded-full border border-border bg-card text-muted-foreground">
        <Music className="size-6" />
      </div>
      <p className="text-base font-medium text-foreground">
        {emptyState.title}
      </p>
      <p className="max-w-xs text-sm text-muted-foreground">
        {emptyState.description}
      </p>
    </div>
  )
}
