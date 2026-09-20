import type { Track } from '@/lib/db'
import type { AudioBufferedRange } from '@/components/audio/AudioProgressBar'
import { AudioProgressBar } from '@/components/audio/AudioProgressBar'
import { AudioMainOperations } from '@/components/audio/operations/AudioMainOperations'
import { AudioTrackDescription } from '@/components/audio/AudioTrackDescription'
import { AudioVolume } from '@/components/audio/AudioVolume'
import { LikeButton } from '@/components/audio/LikeButton'
import { QueueButton } from '@/components/audio/queue/QueueButton'
import { NextButton } from '@/components/audio/operations/buttons/NextButton'
import { PlayPauseButton } from '@/components/audio/operations/buttons/PlayPauseButton'
import { formatTime } from '@/lib/format-time'
import { PreviousButton } from './operations/buttons/PreviousButton'

export interface AudioPlayerBarProps {
  track: Track
  currentTime: number
  duration: number
  bufferedRanges: AudioBufferedRange[]
  showInitialLoading: boolean
  isSeeking?: boolean
  volume: number
  onSeek: (time: number) => void
  onSeekStart: () => void
  onSeekEnd: () => void
  onVolumeChange: (volume: number) => void
  onMuteToggle: () => void
  onOpenDrawer?: () => void
}

export function AudioPlayerBar(props: AudioPlayerBarProps) {
  return (
    <div className="relative flex shrink-0 md:h-24 h-20 w-full flex-col border-t border-border bg-card/80 backdrop-blur-xl">
      <AudioProgressBar
        currentTime={props.currentTime}
        duration={props.duration}
        bufferedRanges={props.bufferedRanges}
        showInitialLoading={props.showInitialLoading}
        isSeeking={props.isSeeking}
        onSeek={props.onSeek}
        onSeekStart={props.onSeekStart}
        onSeekEnd={props.onSeekEnd}
      />

      <div className="flex h-full w-full min-w-0 max-w-full items-center gap-2 px-4 md:grid md:grid-cols-3">
        <div className="relative flex min-w-0 flex-1 items-center gap-3 overflow-hidden md:overflow-visible">
          <AudioTrackDescription track={props.track} />
          <button
            type="button"
            onClick={props.onOpenDrawer}
            aria-label={`Open player for ${props.track.title ?? 'Unknown Title'}`}
            className="absolute inset-0 rounded-lg outline-none focus-visible:ring-2 focus-visible:ring-ring md:hidden"
          />
        </div>

        <div className="flex-col items-center gap-2 hidden md:flex">
          <AudioMainOperations />
          <div className="flex items-center gap-1.5 font-mono text-xs tabular-nums text-muted-foreground">
            {formatTime(props.currentTime)}
            <span className="mx-1 text-muted-foreground/60">/</span>
            {formatTime(props.duration)}
          </div>
        </div>

        <div className="items-center justify-end gap-2 hidden md:flex">
          <QueueButton />
          <LikeButton />
          <AudioVolume
            volume={props.volume}
            onVolumeChange={props.onVolumeChange}
            onMuteToggle={props.onMuteToggle}
          />
        </div>
        <div className="relative z-10 flex shrink-0 items-center gap-3 md:hidden">
          <PreviousButton />
          <PlayPauseButton />
          <NextButton />
        </div>
      </div>
    </div>
  )
}
