import type { Track } from '@/lib/db'
import type { AudioBufferedRange } from '@/components/audio/AudioProgressBar'
import { AudioProgressBar } from '@/components/audio/AudioProgressBar'
import { AudioMainOperations } from '@/components/audio/operations/AudioMainOperations'
import { AudioTrackDescription } from '@/components/audio/AudioTrackDescription'
import { AudioVolume } from '@/components/audio/AudioVolume'
import { LikeButton } from '@/components/audio/LikeButton'
import { QueueButton } from '@/components/audio/queue/QueueButton'
import { formatTime } from '@/lib/format-time'

export interface AudioPlayerBarProps {
  track: Track
  currentTime: number
  duration: number
  bufferedRanges: AudioBufferedRange[]
  showInitialLoading: boolean
  volume: number
  onSeek: (time: number) => void
  onSeekStart: () => void
  onSeekEnd: () => void
  onVolumeChange: (event: React.ChangeEvent<HTMLInputElement>) => void
  onMuteToggle: () => void
}

export function AudioPlayerBar(props: AudioPlayerBarProps) {
  return (
    <div className="relative flex shrink-0 h-24 w-full flex-col border-t border-border bg-card/80 backdrop-blur-xl">
      <AudioProgressBar
        currentTime={props.currentTime}
        duration={props.duration}
        bufferedRanges={props.bufferedRanges}
        showInitialLoading={props.showInitialLoading}
        onSeek={props.onSeek}
        onSeekStart={props.onSeekStart}
        onSeekEnd={props.onSeekEnd}
      />

      <div className="grid grid-cols-3 h-full max-w-full w-full items-center gap-2 px-4">
        <div className="flex items-center gap-3">
          <AudioTrackDescription track={props.track} />
        </div>

        <div className="flex flex-col items-center gap-2">
          <AudioMainOperations />
          <div className="flex items-center gap-1.5 font-mono text-xs tabular-nums text-muted-foreground">
            {formatTime(props.currentTime)}
            <span className="mx-1 text-muted-foreground/60">/</span>
            {formatTime(props.duration)}
          </div>
        </div>

        <div className="flex items-center justify-end gap-2">
          <QueueButton />
          <LikeButton />
          <AudioVolume
            volume={props.volume}
            onVolumeChange={props.onVolumeChange}
            onMuteToggle={props.onMuteToggle}
          />
        </div>
      </div>
    </div>
  )
}
