import { useCallback, useEffect, useRef, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Minimize2, Music } from 'lucide-react'
import type { Track } from '@/lib/db'
import { useCachedThumbnail } from '@/hooks/use-cached-thumbnail'
import { useImagePalette } from '@/hooks/use-image-palette'
import { useArtworkBounce } from '@/hooks/use-artwork-bounce'
import { formatTime } from '@/lib/format-time'
import { cn } from '@/lib/utils'
import { useFullscreenStore } from '@/stores/fullscreen-store'
import { usePlayerStore } from '@/stores/player-store'
import { LikeButton } from '../audio/LikeButton'
import { AudioMainOperations } from '../audio/operations/AudioMainOperations'
import {
  AudioProgressBar,
  type AudioBufferedRange,
} from '../audio/AudioProgressBar'
import { AudioVolume } from '../audio/AudioVolume'
import { QueueButton } from '../audio/queue/QueueButton'

const CONTROLS_HIDE_DELAY = 3000

interface AudioFullscreenPlayerProps {
  track: Track
  currentTime: number
  duration: number
  bufferedRanges: AudioBufferedRange[]
  showInitialLoading: boolean
  volume: number
  onVolumeChange: (volume: number) => void
  onMuteToggle: () => void
  onSeek: (time: number) => void
  onSeekStart: () => void
  onSeekEnd: () => void
  getAudioElement: () => HTMLAudioElement | null
}

export function AudioFullscreenPlayer(props: AudioFullscreenPlayerProps) {
  const { url, failed } = useCachedThumbnail(props.track.id, { quality: 'high' })
  const palette = useImagePalette(url)
  const exitFullscreen = useFullscreenStore(state => state.exitFullscreen)
  const syncFullscreen = useFullscreenStore(state => state.syncFullscreen)
  const queue = usePlayerStore(state => state.queue)
  const [controlsVisible, setControlsVisible] = useState(true)
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const artworkBounceRef = useRef<HTMLDivElement>(null)

  useArtworkBounce({
    trackId: props.track.id,
    elementRef: artworkBounceRef,
    getAudioElement: props.getAudioElement,
  })

  const hideControlsLater = useCallback(() => {
    if (hideTimerRef.current) clearTimeout(hideTimerRef.current)
    hideTimerRef.current = setTimeout(() => setControlsVisible(false), CONTROLS_HIDE_DELAY)
  }, [])

  const showControls = useCallback(() => {
    setControlsVisible(true)
    hideControlsLater()
  }, [hideControlsLater])

  useEffect(() => {
    hideControlsLater()
    const onKeyDown = (event: KeyboardEvent) => {
      showControls()
      if (event.key === 'Escape') exitFullscreen()
    }
    window.addEventListener('keydown', onKeyDown)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
      if (hideTimerRef.current) clearTimeout(hideTimerRef.current)
    }
  }, [exitFullscreen, hideControlsLater, showControls])

  useEffect(() => {
    syncFullscreen()
    let disposed = false
    let unlisten: (() => void) | undefined
    getCurrentWindow()
      .onResized(() => syncFullscreen())
      .then((stop) => {
        if (disposed) stop()
        else unlisten = stop
      })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [syncFullscreen])

  const queuePosition = queue.cursor >= 0 && queue.tracks.length > 1
    ? `${queue.cursor + 1} of ${queue.tracks.length}`
    : null

  return (
    <section
      aria-label="Fullscreen player"
      onPointerMove={showControls}
      onFocusCapture={showControls}
      className={cn(
        'fullscreen-player fixed inset-0 z-200 h-dvh w-dvw overflow-hidden bg-black text-white',
        !controlsVisible && 'cursor-none',
      )}
      style={{
        '--fullscreen-color-one': palette[0],
        '--fullscreen-color-two': palette[1],
        '--fullscreen-color-three': palette[2],
      } as React.CSSProperties}
    >
      {url
        ? (
            <div
              key={`${props.track.id}:${url}`}
              aria-hidden
              className="fullscreen-artwork-backdrop absolute inset-0"
              style={{ backgroundImage: `url("${url}")` }}
            />
          )
        : <div className="absolute inset-0 bg-neutral-950" />}
      <div
        key={`palette:${props.track.id}:${url ?? 'fallback'}`}
        className="fullscreen-palette absolute inset-0"
      />
      <div className="absolute inset-0 bg-black/45" />

      <button
        type="button"
        aria-label="Exit fullscreen player"
        onClick={exitFullscreen}
        className={cn(
          'absolute right-6 top-6 z-20 flex size-10 items-center justify-center rounded-full border border-white/10 bg-black/15 text-white/85 backdrop-blur-xl transition-all hover:bg-white/10 hover:text-white focus-visible:opacity-100',
          controlsVisible ? 'opacity-100' : 'pointer-events-none opacity-0',
        )}
      >
        <Minimize2 className="size-5" />
      </button>

      <div className="relative z-10 flex h-full items-center justify-center px-10 pb-44 pt-12">
        <div
          key={props.track.id}
          className="fullscreen-art-enter aspect-square w-[min(58vh,54vw)] min-w-64"
        >
          <div
            ref={artworkBounceRef}
            className="size-full overflow-hidden rounded-2xl border border-white/10 bg-white/5 shadow-[0_35px_100px_rgba(0,0,0,0.55)]"
          >
            {failed || !url
              ? (
                  <div className="flex size-full items-center justify-center text-muted-foreground">
                    <Music className="size-20" />
                  </div>
                )
              : (
                  <img
                    src={url}
                    alt={`${props.track.title ?? 'Unknown title'} artwork`}
                    className="size-full object-cover"
                  />
                )}
          </div>
        </div>
      </div>

      <div
        className={cn(
          'absolute inset-x-0 bottom-0 z-20 flex justify-center bg-linear-to-t from-black/75 via-black/35 to-transparent px-8 pb-7 pt-24 transition-all duration-500',
          controlsVisible
            ? 'translate-y-0 opacity-100'
            : 'pointer-events-none translate-y-4 opacity-0',
        )}
      >
        <div className="w-full max-w-4xl">
          <div className="mb-5 flex items-end justify-between gap-8">
            <div className="min-w-0">
              <h1 className="truncate text-2xl font-semibold tracking-tight">
                {props.track.title ?? 'Unknown Title'}
              </h1>
              <p className="mt-1 truncate text-sm text-white/65">
                {props.track.performer ?? 'Unknown Artist'}
                {queue.source ? ` · ${queue.source.name}` : ''}
                {queuePosition ? ` · ${queuePosition}` : ''}
              </p>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <LikeButton className="text-white hover:bg-white/10 hover:text-white" />
              <QueueButton
                className="text-white hover:bg-white/10 hover:text-white"
                classes={{ positioner: 'z-[250]' }}
              />
              <AudioVolume
                classes={{ positioner: 'z-[250]' }}
                volume={props.volume}
                onVolumeChange={props.onVolumeChange}
                onMuteToggle={props.onMuteToggle}
              />
            </div>
          </div>

          <div className="relative">
            <AudioProgressBar
              currentTime={props.currentTime}
              duration={props.duration}
              bufferedRanges={props.bufferedRanges}
              showInitialLoading={props.showInitialLoading}
              onSeek={props.onSeek}
              onSeekStart={props.onSeekStart}
              onSeekEnd={props.onSeekEnd}
              className="relative top-auto z-auto h-6"
            />
          </div>

          <div className="mt-2 grid grid-cols-3 items-center">
            <span className="font-mono text-xs tabular-nums text-white/55">
              {formatTime(props.currentTime)}
            </span>
            <div className="flex justify-center">
              <AudioMainOperations />
            </div>
            <span className="text-right font-mono text-xs tabular-nums text-white/55">
              {formatTime(props.duration)}
            </span>
          </div>
        </div>
      </div>
    </section>
  )
}
