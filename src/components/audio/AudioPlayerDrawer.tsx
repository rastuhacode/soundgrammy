import { ChevronDown, Music } from 'lucide-react'
import { useCachedThumbnail } from '@/hooks/use-cached-thumbnail'
import { formatTime } from '@/lib/format-time'
import { usePlayerStore } from '@/stores/player-store'
import {
  Drawer,
  DrawerContent,
  DrawerTitle,
} from '@/components/ui/drawer'
import { AudioProgressBar } from './AudioProgressBar'
import type { AudioPlayerBarProps } from './AudioPlayerBar'
import { AudioMainOperations } from './operations/AudioMainOperations'
import { LikeButton } from './LikeButton'
import { QueueButton } from './queue/QueueButton'
import { Button } from '../ui/button'

interface AudioPlayerDrawerProps extends Omit<AudioPlayerBarProps, 'onOpenDrawer'> {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function AudioPlayerDrawer(props: AudioPlayerDrawerProps) {
  const { url, failed, onError } = useCachedThumbnail(props.track.id, { quality: 'high' })
  const queueSource = usePlayerStore(state => state.queue.source?.name)

  return (
    <Drawer open={props.open} onOpenChange={props.onOpenChange}>
      <DrawerContent
        className="overflow-hidden rounded-none border-none p-0"
        style={{
          '--drawer-inset': '0px',
          '--drawer-height': '100dvh',
          '--drawer-content-max-height': '100dvh',
        } as React.CSSProperties}
      >
        <div className="flex min-h-0 grow flex-col">
          <header className="android-overlay-inset flex shrink-0 items-center justify-between gap-3 px-5 pt-3">
            <Button
              onClick={() => props.onOpenChange(false)}
              variant="ghost"
              aria-label="Close player"
              className="flex size-11 shrink-0 items-center justify-center text-foreground hover:bg-muted/70 focus-visible:outline-2 focus-visible:outline-ring"
            >
              <ChevronDown className="size-6" />
            </Button>
            <div className="min-w-0 text-center">
              <DrawerTitle className="sr-only">
                {props.track.title ?? 'Unknown Title'}
              </DrawerTitle>
              <p className="text-[10px] font-semibold tracking-widest text-muted-foreground uppercase">
                Now playing
              </p>
              <p className="truncate text-sm font-medium text-foreground">
                {queueSource ?? 'Queue'}
              </p>
            </div>
            <QueueButton
              className="size-11"
              classes={{ positioner: 'z-[80]' }}
            />
          </header>

          <div className="flex min-h-0 grow items-center justify-center overflow-hidden px-7 py-5 @container-size">
            <div className="aspect-square w-[min(100cqw,100cqh)] shrink-0 overflow-hidden rounded-2xl bg-muted shadow-2xl ring-1 ring-border">
              {failed || !url
                ? (
                    <div className="flex size-full items-center justify-center text-muted-foreground">
                      <Music className="size-16" />
                    </div>
                  )
                : (
                    <img
                      src={url}
                      alt={`${props.track.title ?? 'Unknown title'} artwork`}
                      onError={onError}
                      className="size-full object-cover"
                    />
                  )}
            </div>
          </div>

          <div className="shrink-0 px-6 pb-[max(1.25rem,env(safe-area-inset-bottom))]">
            <div className="flex min-w-0 items-center justify-between gap-3">
              <div className="min-w-0">
                <h2 className="truncate text-xl font-semibold text-foreground">
                  {props.track.title ?? 'Unknown Title'}
                </h2>
                <p className="truncate text-sm text-muted-foreground">
                  {props.track.performer ?? 'Unknown Artist'}
                </p>
              </div>
              <LikeButton className="size-11 shrink-0" />
            </div>

            <AudioProgressBar
              currentTime={props.currentTime}
              duration={props.duration}
              bufferedRanges={props.bufferedRanges}
              showInitialLoading={props.showInitialLoading}
              isSeeking={props.isSeeking}
              onSeek={props.onSeek}
              onSeekStart={props.onSeekStart}
              onSeekEnd={props.onSeekEnd}
              className="relative top-auto z-auto h-7"
            />
            <div className="flex justify-between font-mono text-xs tabular-nums text-muted-foreground">
              <span>{formatTime(props.currentTime)}</span>
              <span>{formatTime(props.duration)}</span>
            </div>

            <div className="flex justify-center">
              <AudioMainOperations />
            </div>
          </div>
        </div>
      </DrawerContent>
    </Drawer>
  )
}
