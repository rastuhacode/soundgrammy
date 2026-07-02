import { useRef, useState, useEffect, useMemo } from 'react'
import { useLocalStorage } from '@mantine/hooks'
import { Heart, Loader2 } from 'lucide-react'
import { AudioProgressBar } from '@/components/audio/AudioProgressBar'
import { usePlayerStore } from '@/stores/player-store'
import { useRepeatStore } from '@/stores/repeat-store'
import { useShuffleStore } from '@/stores/shuffle-store'
import { usePlaylistsStore } from '@/stores/playlists-store'
import { Button } from '@/components/ui/button'
import { api, fileSrc } from '@/lib/api'
import { cn } from '@/lib/utils'
import { AudioMainOperations } from './AudioMainOperations'
import { AudioTrackDescription } from './AudioTrackDescription'
import { AudioVolume } from './AudioVolume'

const VOLUME_STORAGE_KEY = 'soundgrammy-volume'
const VOLUME_DEFAULT = 25 // 25% (players shouldn't scream by default)

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds)) return '0:00'
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

function parseStoredVolume(stored: string | undefined): number {
  if (stored === undefined) return VOLUME_DEFAULT
  let value: unknown
  try {
    value = JSON.parse(stored)
  }
  catch {
    value = Number(stored)
  }
  const n = typeof value === 'number' ? value : Number(value)
  return Number.isFinite(n) && n >= 0 && n <= 100 ? n : VOLUME_DEFAULT
}

export function AudioPlayer() {
  const track = usePlayerStore(state => state.currentTrack)
  const isPlaying = usePlayerStore(state => state.isPlaying)
  const setPlaying = usePlayerStore(state => state.setPlaying)
  const playNext = usePlayerStore(state => state.playNext)
  const playPrevious = usePlayerStore(state => state.playPrevious)
  const repeat = useRepeatStore(state => state.repeat)
  const toggleRepeat = useRepeatStore(state => state.toggleRepeat)
  const shuffle = useShuffleStore(state => state.shuffle)
  const toggleShuffle = usePlayerStore(state => state.toggleShuffle)
  const hydratePreferences = usePlayerStore(
    state => state.hydratePreferences,
  )
  const playlistsData = usePlaylistsStore(state => state.data)
  const setPlaylistsData = usePlaylistsStore(state => state.setData)

  const [currentTime, setCurrentTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const [bufferedTime, setBufferedTime] = useState(0)
  const [isBuffering, setIsBuffering] = useState(false)
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
  const loadGenerationRef = useRef(0)
  const isPlayingRef = useRef(isPlaying)

  const isLiked = useMemo(() => {
    return playlistsData?.liked.trackIds.includes(track?.id ?? 0) ?? false
  }, [playlistsData, track?.id])

  const playAudio = (audio: HTMLAudioElement, generation: number) => {
    void audio
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

  const handleSeekEnd = () => {
    isSeekingRef.current = false
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

  // Load (and, on first play, download+cache) the current track from disk.
  useEffect(() => {
    const audio = audioRef.current
    if (!audio || !track) return

    const generation = ++loadGenerationRef.current

    setCurrentTime(0)
    setDuration(0)
    setBufferedTime(0)
    setIsBuffering(true)

    audio.pause()

    void api
      .getTrackSource(track.id)
      .then((path) => {
        if (loadGenerationRef.current !== generation) return
        audio.src = fileSrc(path)
        applyVolume()
        audio.load()
        setIsBuffering(false)
        if (isPlayingRef.current) {
          playAudio(audio, generation)
        }
      })
      .catch(() => {
        if (loadGenerationRef.current === generation) {
          setIsBuffering(false)
          setPlaying(false)
        }
      })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [track?.id])

  useEffect(() => {
    const audio = audioRef.current
    if (!audio || !track || !audio.src) return

    const generation = loadGenerationRef.current

    if (isPlaying) {
      playAudio(audio, generation)
    }
    else {
      loadGenerationRef.current += 1
      audio.pause()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying, setPlaying])

  const onTimeUpdate = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    if (isSeekingRef.current) return
    setCurrentTime(event.currentTarget.currentTime)
  }

  const onDurationChange = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    setDuration(event.currentTarget.duration)
  }

  const onProgress = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    const audio = event.currentTarget
    if (!audio.buffered.length) {
      setBufferedTime(0)
      return
    }
    setBufferedTime(audio.buffered.end(audio.buffered.length - 1))
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
      playAudio(audio, loadGenerationRef.current)
      return
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
        onTimeUpdate={onTimeUpdate}
        onDurationChange={onDurationChange}
        onProgress={onProgress}
        onEnded={handleTrackEnded}
        className="hidden"
      />

      {track
        ? (
            <div className="relative flex h-24 w-full flex-col border-t border-border bg-card/80 backdrop-blur-xl">
              <AudioProgressBar
                currentTime={currentTime}
                duration={duration}
                bufferedTime={bufferedTime}
                onSeek={(time) => {
                  if (!audioRef.current) return
                  isSeekingRef.current = true
                  audioRef.current.currentTime = time
                  setCurrentTime(time)
                }}
                onSeekStart={() => {
                  isSeekingRef.current = true
                }}
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
                    {isBuffering
                      ? (
                          <Loader2 className="size-3 animate-spin text-primary" />
                        )
                      : null}
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
