import {
  useMemo,
  useState,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from 'react'
import type { DownloadProgress } from '@/lib/api'
import {
  computeBufferedRanges,
  toCachedRanges,
  type BufferAnchor,
} from './buffer-ranges'

export interface UseAudioSeekOptions {
  audioRef: RefObject<HTMLAudioElement | null>
  downloadProgress: DownloadProgress | null
  duration: number
  currentTime: number
  setCurrentTime: Dispatch<SetStateAction<number>>
  trackId: number | undefined
  isPlayingRef: RefObject<boolean>
  loadedTrackIdRef: RefObject<number | null>
  loadGenerationRef: RefObject<number>
  pendingSeekRef: RefObject<number | null>
  resumeAfterSeekRef: RefObject<boolean>
  isSeekingRef: RefObject<boolean>
  playAudio: (audio: HTMLAudioElement, generation: number) => void
}

export function useAudioSeek(options: UseAudioSeekOptions) {
  const {
    audioRef,
    downloadProgress,
    duration,
    currentTime,
    setCurrentTime,
    trackId,
    isPlayingRef,
    loadedTrackIdRef,
    loadGenerationRef,
    pendingSeekRef,
    resumeAfterSeekRef,
    isSeekingRef,
    playAudio,
  } = options

  const [bufferAnchor, setBufferAnchor] = useState<BufferAnchor | null>(null)
  const [anchorTrackId, setAnchorTrackId] = useState(trackId)
  if (trackId !== anchorTrackId) {
    setAnchorTrackId(trackId)
    setBufferAnchor(null)
  }

  const cachedRanges = useMemo(
    () => toCachedRanges(downloadProgress, duration),
    [downloadProgress, duration],
  )

  const bufferedRanges = useMemo(
    () => computeBufferedRanges({
      cachedRanges,
      bufferAnchor,
      downloadProgress,
      duration,
      currentTime,
    }),
    [bufferAnchor, cachedRanges, currentTime, downloadProgress, duration],
  )

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
    if (shouldResume && loadedTrackIdRef.current === trackId) {
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

  return {
    bufferedRanges,
    handleSeek,
    handleSeekStart,
    handleSeekEnd,
    finishPendingSeek,
    onLoadedMetadata,
    onSeeked,
    onCanPlay,
  }
}
