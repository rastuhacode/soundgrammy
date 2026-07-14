import { useRef, useState, useEffect, useMemo } from 'react'
import { useLocalStorage } from '@mantine/hooks'
import { Heart } from 'lucide-react'
import { AudioProgressBar } from '@/components/audio/AudioProgressBar'
import { usePlayerStore } from '@/stores/player-store'
import { useRepeatStore } from '@/stores/repeat-store'
import { useShuffleStore } from '@/stores/shuffle-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { useFullscreenStore } from '@/stores/fullscreen-store'
import { Button } from '@/components/ui/button'
import {
  api,
  fileSrc,
  onDownloadProgress,
  streamSrc,
  type DownloadProgress,
} from '@/lib/api'
import { cn } from '@/lib/utils'
import { AudioMainOperations } from './operations/AudioMainOperations'
import { AudioFullscreenPlayer } from '../fullscreen/AudioFullscreenPlayer'
import { AudioTrackDescription } from './AudioTrackDescription'
import { AudioVolume } from './AudioVolume'
import { z } from 'zod'

const VOLUME_STORAGE_KEY = 'soundgrammy-volume'
const VOLUME_DEFAULT = 25 // 25% (player shouldn't scream by default)

interface BufferAnchor {
  time: number
  received: number
}

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds)) return '0:00'
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

function parseStoredVolume(stored: string | undefined): number {
  const volumeSchema = z.number().min(0).max(100).default(VOLUME_DEFAULT)

  if (stored === undefined) return VOLUME_DEFAULT
  let value: number | undefined
  try {
    value = volumeSchema.safeParse(JSON.parse(stored)).data
  }
  catch {
    value = volumeSchema.safeParse(Number(stored)).data
  }
  return value ?? VOLUME_DEFAULT
}

export function AudioPlayer() {
  const track = usePlayerStore(state => state.currentTrack)
  const isPlaying = usePlayerStore(state => state.isPlaying)
  const setPlaying = usePlayerStore(state => state.setPlaying)
  const playNext = usePlayerStore(state => state.playNext)
  const playPrevious = usePlayerStore(state => state.playPrevious)
  const repeat = useRepeatStore(state => state.repeat)
  const toggleRepeat = usePlayerStore(state => state.toggleRepeat)
  const shuffle = useShuffleStore(state => state.shuffle)
  const toggleShuffle = usePlayerStore(state => state.toggleShuffle)
  const hydratePreferences = usePlayerStore(
    state => state.hydratePreferences,
  )
  const playlistsData = usePlaylistsStore(state => state.data)
  const setPlaylistsData = usePlaylistsStore(state => state.setData)
  const isFullscreen = useFullscreenStore(state => state.isFullscreen)
  const exitFullscreen = useFullscreenStore(state => state.exitFullscreen)

  const [currentTime, setCurrentTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const [downloadProgress, setDownloadProgress]
    = useState<DownloadProgress | null>(null)
  const [showInitialLoading, setShowInitialLoading] = useState(false)
  const [bufferAnchor, setBufferAnchor] = useState<BufferAnchor | null>(null)
  const [volume, setVolume] = useLocalStorage<number>({
    key: VOLUME_STORAGE_KEY,
    defaultValue: VOLUME_DEFAULT,
    getInitialValueInEffect: false,
    deserialize: parseStoredVolume,
  })
  const [isMuted, setIsMuted] = useState(false)
  const audioRef = useRef<HTMLAudioElement>(null)
  const volumeRef = useRef(volume)
  const preMuteVolumeRef = useRef(volume)
  const isSeekingRef = useRef(false)
  const pendingSeekRef = useRef<number | null>(null)
  const resumeAfterSeekRef = useRef(false)
  const loadGenerationRef = useRef(0)
  const loadedTrackIdRef = useRef<number | null>(null)
  const isPlayingRef = useRef(isPlaying)

  const isLiked = useMemo(() => {
    return playlistsData?.liked.trackIds.includes(track?.id ?? 0) ?? false
  }, [playlistsData, track?.id])
  const cachedRanges = useMemo(() => {
    return duration > 0 && downloadProgress?.total
      ? downloadProgress.ranges.map(range => ({
          start: (range.start / downloadProgress.total) * duration,
          end: (range.end / downloadProgress.total) * duration,
        }))
      : []
  }, [downloadProgress, duration])
  const bufferedRanges = useMemo(() => {
    const leadingRange = cachedRanges.find(range => range.start === 0)
    const leadingIncludesAnchor = bufferAnchor !== null
      && leadingRange
      && bufferAnchor.time < leadingRange.end

    if (bufferAnchor !== null && !leadingIncludesAnchor) {
      const receivedSinceSeek = Math.max(
        0,
        (downloadProgress?.received ?? bufferAnchor.received)
        - bufferAnchor.received,
      )
      if (receivedSinceSeek > 0 && downloadProgress?.total) {
        const bufferedDuration
          = (receivedSinceSeek / downloadProgress.total) * duration
        return [{
          start: bufferAnchor.time,
          end: Math.min(duration, bufferAnchor.time + bufferedDuration),
        }]
      }
      return []
    }

    const activeRange = cachedRanges.find(
      range => currentTime >= range.start && currentTime < range.end,
    )
    const displayedRange = activeRange ?? leadingRange ?? cachedRanges[0]
    return displayedRange ? [displayedRange] : []
  }, [bufferAnchor, cachedRanges, currentTime, downloadProgress, duration])

  const playAudio = (audio: HTMLAudioElement, generation: number) => {
    audio
      .play()
      .then(() => {
        if (loadGenerationRef.current !== generation || !isPlayingRef.current) {
          audio.pause()
        }
      })
      .catch(() => {
        if (loadGenerationRef.current === generation) {
          setPlaying(false)
        }
      })
  }

  const applyVolume = () => {
    const audio = audioRef.current
    if (!audio) return
    audio.volume = volumeRef.current / 100
  }

  const setAudioRef = (node: HTMLAudioElement | null) => {
    audioRef.current = node
    if (node) {
      node.volume = volumeRef.current / 100
    }
  }

  const togglePlay = () => {
    if (!track) return
    setPlaying(!isPlaying)
  }

  const finishPendingSeek = (audio: HTMLAudioElement) => {
    if (
      isSeekingRef.current
      || pendingSeekRef.current === null
      || audio.seeking
      || audio.readyState < HTMLMediaElement.HAVE_CURRENT_DATA
    ) {
      return
    }

    pendingSeekRef.current = null
    setCurrentTime(audio.currentTime)
    const shouldResume = resumeAfterSeekRef.current && isPlayingRef.current
    resumeAfterSeekRef.current = false
    if (shouldResume && loadedTrackIdRef.current === track?.id) {
      playAudio(audio, loadGenerationRef.current)
    }
  }

  const handleSeekStart = () => {
    isSeekingRef.current = true
    setBufferAnchor(null)
  }

  const handleSeekEnd = () => {
    isSeekingRef.current = false
    const audio = audioRef.current
    if (audio) finishPendingSeek(audio)
  }

  const handleSeek = (time: number) => {
    const audio = audioRef.current
    if (!audio) return

    const isCached = cachedRanges.some(
      range => time >= range.start && time < range.end,
    )
    setBufferAnchor(current => isCached
      ? null
      : {
          time,
          received: current?.received ?? downloadProgress?.received ?? 0,
        })
    if (!isCached && !resumeAfterSeekRef.current) {
      resumeAfterSeekRef.current = isPlayingRef.current
      audio.pause()
    }

    pendingSeekRef.current = time
    if (audio.readyState > HTMLMediaElement.HAVE_NOTHING) {
      audio.currentTime = time
    }
    setCurrentTime(time)
  }

  const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setIsMuted(false)
    const nextVolume = Number(e.target.value)
    setVolume(nextVolume)
    volumeRef.current = nextVolume
    applyVolume()
  }

  const handleMuteToggle = () => {
    if (isMuted) {
      const restored = preMuteVolumeRef.current
      setVolume(restored)
      volumeRef.current = restored
      setIsMuted(false)
    }
    else {
      preMuteVolumeRef.current = volume
      setVolume(0)
      volumeRef.current = 0
      setIsMuted(true)
    }
    applyVolume()
  }

  useEffect(hydratePreferences, [hydratePreferences])
  useEffect(() => {
    volumeRef.current = volume
    applyVolume()
  }, [volume])
  useEffect(() => {
    isPlayingRef.current = isPlaying
  }, [isPlaying])
  useEffect(() => {
    if (!track && isFullscreen) exitFullscreen()
  }, [exitFullscreen, isFullscreen, track])
  // Resolve a complete local file or attach the progressive range protocol.
  useEffect(() => {
    const audio = audioRef.current
    const generation = ++loadGenerationRef.current
    let disposed = false
    let unlisten: (() => void) | undefined

    loadedTrackIdRef.current = null

    queueMicrotask(() => {
      if (loadGenerationRef.current !== generation) return
      setCurrentTime(0)
      setDuration(track?.duration ?? 0)
      setDownloadProgress(null)
      setShowInitialLoading(Boolean(audio && track))
      setBufferAnchor(null)
    })

    pendingSeekRef.current = null
    resumeAfterSeekRef.current = false
    isSeekingRef.current = false
    if (!audio) return

    audio.pause()
    audio.removeAttribute('src')
    audio.load()

    if (!track) return

    const initializeSource = async () => {
      try {
        const stop = await onDownloadProgress((progress) => {
          if (
            !disposed
            && loadGenerationRef.current === generation
            && progress.trackId === track.id
          ) {
            setDownloadProgress(progress)
            if (progress.received > 0) {
              setShowInitialLoading(false)
            }
          }
        })
        if (disposed || loadGenerationRef.current !== generation) {
          stop()
          return
        }
        unlisten = stop

        const source = await api.getTrackSource(track.id)
        if (disposed || loadGenerationRef.current !== generation) return
        if (source.kind === 'cached') {
          const total = track.file_size ?? 1
          setDownloadProgress({
            trackId: track.id,
            received: total,
            total,
            ranges: [{ start: 0, end: total }],
            complete: true,
          })
          setShowInitialLoading(false)
          audio.src = fileSrc(source.path)
        }
        else {
          setDownloadProgress(current =>
            current?.trackId === source.trackId
              ? current
              : {
                  trackId: source.trackId,
                  received: 0,
                  total: source.total,
                  ranges: [],
                  complete: false,
                },
          )
          audio.src = streamSrc(source.trackId)
        }
        loadedTrackIdRef.current = track.id
        applyVolume()
        audio.load()
        if (isPlayingRef.current) {
          if (pendingSeekRef.current === null) {
            playAudio(audio, generation)
          }
          else {
            resumeAfterSeekRef.current = true
          }
        }
      }
      catch {
        if (!disposed && loadGenerationRef.current === generation) {
          setShowInitialLoading(false)
          setPlaying(false)
        }
      }
    }
    initializeSource()

    return () => {
      disposed = true
      unlisten?.()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [track?.id])

  useEffect(() => {
    const audio = audioRef.current
    if (!audio || !track || !audio.src) return

    const generation = loadGenerationRef.current

    if (isPlaying && loadedTrackIdRef.current === track.id) {
      if (pendingSeekRef.current === null) {
        playAudio(audio, generation)
      }
      else {
        resumeAfterSeekRef.current = true
        finishPendingSeek(audio)
      }
    }
    else {
      resumeAfterSeekRef.current = false
      audio.pause()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying, setPlaying])

  const onTimeUpdate = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    if (isSeekingRef.current || pendingSeekRef.current !== null) return
    setCurrentTime(event.currentTarget.currentTime)
  }

  const onDurationChange = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    const nextDuration = event.currentTarget.duration
    if (Number.isFinite(nextDuration) && nextDuration > 0) {
      setDuration(nextDuration)
    }
  }

  const onLoadedMetadata = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    const pendingSeek = pendingSeekRef.current
    if (pendingSeek === null) return
    event.currentTarget.currentTime = pendingSeek
  }

  const onSeeked = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    finishPendingSeek(event.currentTarget)
  }

  const onCanPlay = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    finishPendingSeek(event.currentTarget)
  }

  const onMediaError = () => {
    if (loadedTrackIdRef.current !== track?.id) return
    setShowInitialLoading(false)
    pendingSeekRef.current = null
    resumeAfterSeekRef.current = false
    setPlaying(false)
  }

  function handlePreviousTrack() {
    const PREVIOUS_TRACK_THRESHOLD = 5 // 5s
    if (currentTime < PREVIOUS_TRACK_THRESHOLD) {
      playPrevious()
    }
    else {
      const audio = audioRef.current
      if (audio) {
        audio.currentTime = 0
      }
      setCurrentTime(0)
    }
  }

  function handleTrackEnded() {
    if (repeat === 'one') {
      const audio = audioRef.current
      if (!audio) return
      audio.currentTime = 0
      return playAudio(audio, loadGenerationRef.current)
    }
    playNext()
  }

  async function handleToggleLike() {
    if (!track || !playlistsData) return
    try {
      const trackIds = await api.toggleLike(track.id)
      setPlaylistsData({
        ...playlistsData,
        liked: { ...playlistsData.liked, trackIds },
      })
    }
    catch {
      // keep UI unchanged on failure
    }
  }

  return (
    <>
      <audio
        ref={setAudioRef}
        preload="metadata"
        onLoadedMetadata={onLoadedMetadata}
        onSeeked={onSeeked}
        onCanPlay={onCanPlay}
        onTimeUpdate={onTimeUpdate}
        onDurationChange={onDurationChange}
        onEnded={handleTrackEnded}
        onError={onMediaError}
        className="hidden"
      />

      {track && isFullscreen
        ? (
            <AudioFullscreenPlayer
              track={track}
              isPlaying={isPlaying}
              currentTime={currentTime}
              duration={duration}
              bufferedRanges={bufferedRanges}
              showInitialLoading={showInitialLoading}
              volume={volume}
              isLiked={isLiked}
              repeatState={repeat}
              shuffleState={shuffle}
              onPlayToggle={togglePlay}
              onPreviousTrack={handlePreviousTrack}
              onNextTrack={playNext}
              onRepeatToggle={toggleRepeat}
              onShuffleToggle={toggleShuffle}
              onLikeToggle={handleToggleLike}
              onVolumeChange={handleVolumeChange}
              onMuteToggle={handleMuteToggle}
              onSeek={handleSeek}
              onSeekStart={handleSeekStart}
              onSeekEnd={handleSeekEnd}
            />
          )
        : null}

      {track && !isFullscreen
        ? (
            <div className="relative flex h-24 w-full flex-col border-t border-border bg-card/80 backdrop-blur-xl">
              <AudioProgressBar
                currentTime={currentTime}
                duration={duration}
                bufferedRanges={bufferedRanges}
                showInitialLoading={showInitialLoading}
                onSeek={handleSeek}
                onSeekStart={handleSeekStart}
                onSeekEnd={handleSeekEnd}
              />

              <div className="flex h-full w-full items-center gap-2 px-4">
                <div className="flex w-1/3 items-center gap-3">
                  <AudioTrackDescription track={track} />
                </div>

                <div className="flex w-1/3 flex-col items-center gap-2">
                  <AudioMainOperations
                    isPlaying={isPlaying}
                    onPlayToggle={togglePlay}
                    onPreviousTrack={handlePreviousTrack}
                    onNextTrack={playNext}
                    repeatState={repeat}
                    onRepeatToggle={toggleRepeat}
                    shuffleState={shuffle}
                    onShuffleToggle={toggleShuffle}
                  />
                  <div className="flex items-center gap-1.5 font-mono text-xs tabular-nums text-muted-foreground">
                    {formatTime(currentTime)}
                    <span className="mx-1 text-muted-foreground/60">/</span>
                    {formatTime(duration)}
                  </div>
                </div>

                <div className="w-1/3 items-center gap-2 flex justify-end">
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label={isLiked ? 'Remove from liked' : 'Add to liked'}
                    onClick={handleToggleLike}
                  >
                    <Heart className={cn('size-5', isLiked && 'fill-current')} />
                  </Button>
                  <AudioVolume
                    volume={volume}
                    onVolumeChange={handleVolumeChange}
                    onMuteToggle={handleMuteToggle}
                  />
                </div>
              </div>
            </div>
          )
        : null}
    </>
  )
}
