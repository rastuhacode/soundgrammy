import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from 'react'
import type { DownloadProgress } from '@/lib/api'
import { appendedEndTime, leadingPrefixEnd } from './mse-append-queue'
import {
  computeBufferedRanges,
  landTimeOnRanges,
  snapTimeToRanges,
  toCachedRanges,
  type TimeRange,
} from './buffer-ranges'

export interface UseAudioSeekOptions {
  audioRef: RefObject<HTMLAudioElement | null>
  downloadProgress: DownloadProgress | null
  /** Bytes successfully handed to SourceBuffer (0 for non-MSE / unknown). */
  appendedBytes: number
  /** Bumps when SourceBuffer buffered ranges change. */
  bufferRevision: number
  /** Discontinuous MSE seek; no-op when not on an MSE stream. */
  seekMseToTime: (time: number) => Promise<void>
  /**
   * Snap onto SourceBuffer ranges. When present, preferred over
   * HTMLMediaElement.buffered (which can stay stale after remove on WebKit).
   */
  mseSnapToBufferedTime?: (time: number) => number | null
  /** Land onto the active SourceBuffer island after discontinuous append. */
  mseLandToBufferedTime?: (time: number) => number | null
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

function mediaBufferedRanges(audio: HTMLAudioElement): TimeRange[] {
  const ranges: TimeRange[] = []
  for (let i = 0; i < audio.buffered.length; i++) {
    ranges.push({
      start: audio.buffered.start(i),
      end: audio.buffered.end(i),
    })
  }
  return ranges
}

function rangesEqual(a: TimeRange[], b: TimeRange[]): boolean {
  if (a.length !== b.length) return false
  for (let i = 0; i < a.length; i++) {
    if (a[i]!.start !== b[i]!.start || a[i]!.end !== b[i]!.end) return false
  }
  return true
}

export function useAudioSeek(options: UseAudioSeekOptions) {
  const {
    audioRef,
    downloadProgress,
    appendedBytes,
    bufferRevision,
    seekMseToTime,
    mseSnapToBufferedTime,
    mseLandToBufferedTime,
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

  const [mediaRanges, setMediaRanges] = useState<TimeRange[]>([])
  const discontinuitySeekRef = useRef(false)
  const mediaRangesRef = useRef<TimeRange[]>([])

  useEffect(() => {
    mediaRangesRef.current = mediaRanges
  }, [mediaRanges])

  const syncMediaRanges = (audio: HTMLAudioElement) => {
    const next = mediaBufferedRanges(audio)
    if (rangesEqual(mediaRangesRef.current, next)) return next
    mediaRangesRef.current = next
    setMediaRanges(next)
    return next
  }

  // Sync honest MSE buffered ranges when the SourceBuffer actually changes.
  useEffect(() => {
    const audio = audioRef.current
    if (!audio) {
      if (mediaRangesRef.current.length > 0) {
        mediaRangesRef.current = []
        setMediaRanges([])
      }
      return
    }
    syncMediaRanges(audio)
  }, [appendedBytes, audioRef, bufferRevision, trackId])

  const cachedRanges = useMemo(
    () => toCachedRanges(downloadProgress, duration),
    [downloadProgress, duration],
  )

  const playableEnd = useMemo(() => {
    if (downloadProgress?.complete) return duration
    if (mediaRanges.length > 0) {
      return Math.min(
        duration,
        Math.max(...mediaRanges.map(range => range.end)),
      )
    }
    // Mid-file append cursors must not paint a fake 0..t prefix.
    if (downloadProgress?.total && appendedBytes > 0) {
      const prefixEnd = leadingPrefixEnd(downloadProgress.ranges)
      if (prefixEnd > 0 && appendedBytes <= prefixEnd) {
        return appendedEndTime(
          appendedBytes,
          downloadProgress.total,
          duration,
        )
      }
    }
    if (!downloadProgress && appendedBytes > 0 && duration > 0) {
      return duration
    }
    return 0
  }, [appendedBytes, downloadProgress, duration, mediaRanges])

  const bufferedRanges = useMemo(
    () => computeBufferedRanges({
      mediaRanges,
      playableEnd,
      duration,
      currentTime,
      cachedRanges,
    }),
    [cachedRanges, currentTime, duration, mediaRanges, playableEnd],
  )

  const finishPendingSeek = (audio: HTMLAudioElement) => {
    if (
      isSeekingRef.current
      || pendingSeekRef.current === null
      || audio.readyState < HTMLMediaElement.HAVE_CURRENT_DATA
    ) {
      return
    }

    const pending = pendingSeekRef.current
    syncMediaRanges(audio)

    let landed = pending
    if (!downloadProgress?.complete) {
      const snapped = landSeekTime(pending, mediaBufferedRanges(audio))
      if (snapped === null) return
      landed = snapped
      if (Math.abs(audio.currentTime - landed) > 0.05) {
        try {
          audio.currentTime = landed
        }
        catch {
          return
        }
        if (audio.seeking) return
      }
      else if (audio.seeking) {
        return
      }
    }
    else if (audio.seeking) {
      return
    }

    pendingSeekRef.current = null
    resumeAfterSeekRef.current = false
    setCurrentTime((current) => {
      return Math.abs(current - landed) > 0.05 ? landed : current
    })
    // Store isPlaying is the source of truth — resume even if resumeAfterSeek
    // was cleared while the element stayed paused after a discontinuous seek.
    if (isPlayingRef.current && loadedTrackIdRef.current === trackId) {
      playAudio(audio, loadGenerationRef.current)
    }
  }

  const handleSeekStart = () => {
    isSeekingRef.current = true
  }

  /**
   * Discontinuous MSE seek that chases `pendingSeekRef`.
   * While a seek is in flight, scrub only updates the pending time; when the
   * current island lands we loop if the user has moved further.
   */
  const startDiscontinuitySeek = () => {
    if (discontinuitySeekRef.current) return
    discontinuitySeekRef.current = true

    void (async () => {
      try {
        while (pendingSeekRef.current !== null) {
          const target = pendingSeekRef.current
          const el = audioRef.current
          if (!el) return

          await seekMseToTime(target)

          const audio = audioRef.current
          if (!audio || pendingSeekRef.current === null) return

          const pending = pendingSeekRef.current
          // Scrub moved while in flight — seek again to the latest pending time.
          if (Math.abs(pending - target) > 0.25) {
            continue
          }

          syncMediaRanges(audio)
          const snapped = downloadProgress?.complete
            ? pending
            : landSeekTime(pending, mediaBufferedRanges(audio))

          if (snapped === null) {
            // Island not visible on SourceBuffer yet — canplay / range sync
            // will finish once the buffer reports it.
            return
          }

          pendingSeekRef.current = snapped
          try {
            audio.currentTime = snapped
          }
          catch {
            // canplay will retry
          }
          finishPendingSeek(audio)
          // finishPendingSeek may bail on readyState/seeking — still resume
          // once data is present so isPlaying cannot desync from audio.paused.
          if (
            isPlayingRef.current
            && audio.paused
            && audio.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA
            && loadedTrackIdRef.current === trackId
          ) {
            pendingSeekRef.current = null
            resumeAfterSeekRef.current = false
            playAudio(audio, loadGenerationRef.current)
          }
          return
        }
      }
      finally {
        discontinuitySeekRef.current = false
      }
    })()
  }

  /** Scrub playable check — tight snap so distant stale islands are not playable. */
  const landPlayableTime = (time: number, ranges: TimeRange[]): number | null => {
    if (downloadProgress?.complete) return time
    // Streamed MSE: trust SourceBuffer only — element buffered can lag after remove.
    if (mseSnapToBufferedTime && downloadProgress && !downloadProgress.complete) {
      return mseSnapToBufferedTime(time)
    }
    return snapTimeToRanges(time, ranges)
  }

  /** Finish/land after discontinuity — SourceBuffer island, not element buffered. */
  const landSeekTime = (time: number, ranges: TimeRange[]): number | null => {
    if (downloadProgress?.complete) return time
    if (mseLandToBufferedTime && downloadProgress && !downloadProgress.complete) {
      return mseLandToBufferedTime(time)
    }
    return landTimeOnRanges(time, ranges)
  }

  const handleSeekEnd = () => {
    isSeekingRef.current = false
    const audio = audioRef.current
    if (!audio) return

    const pending = pendingSeekRef.current
    if (pending !== null) {
      const ranges = mediaBufferedRanges(audio)
      if (landPlayableTime(pending, ranges) === null) {
        startDiscontinuitySeek()
        return
      }
    }
    finishPendingSeek(audio)
  }

  const handleSeek = (time: number) => {
    const audio = audioRef.current
    if (!audio) return

    const ranges = mediaBufferedRanges(audio)
    const landed = landPlayableTime(time, ranges)

    if (landed === null) {
      // Always refresh resume intent from store play state. A prior seek may
      // have cleared resumeAfterSeek while isPlaying stayed true.
      if (isPlayingRef.current) {
        resumeAfterSeekRef.current = true
      }
      audio.pause()
    }

    pendingSeekRef.current = time
    setCurrentTime(time)

    if (landed !== null) {
      if (audio.readyState > HTMLMediaElement.HAVE_NOTHING) {
        audio.currentTime = landed
      }
      return
    }

    // Unbuffered seek → discontinuous MSE island at the target.
    startDiscontinuitySeek()
  }

  const onLoadedMetadata = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    // Discontinuity owns landing until seekMseToTime finishes — element
    // events can still reflect the pre-clear island on WebKit.
    if (discontinuitySeekRef.current) return
    const audio = event.currentTarget
    syncMediaRanges(audio)
    const pendingSeek = pendingSeekRef.current
    if (pendingSeek === null) return
    if (!downloadProgress?.complete) {
      const snapped = landSeekTime(pendingSeek, mediaBufferedRanges(audio))
      if (snapped === null) return
      audio.currentTime = snapped
      return
    }
    audio.currentTime = pendingSeek
  }

  const onSeeked = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    if (discontinuitySeekRef.current) return
    finishPendingSeek(event.currentTarget)
  }

  const onCanPlay = (event: React.SyntheticEvent<HTMLAudioElement>) => {
    if (discontinuitySeekRef.current) return
    finishPendingSeek(event.currentTarget)
  }

  // Land a pending seek once a buffered island covers (or nearly covers) the target.
  useEffect(() => {
    const pending = pendingSeekRef.current
    const audio = audioRef.current
    if (pending === null || !audio || discontinuitySeekRef.current) return
    if (!downloadProgress?.complete) {
      const snapped = landSeekTime(pending, mediaRanges)
      if (snapped === null) return
      if (audio.readyState > HTMLMediaElement.HAVE_NOTHING) {
        if (Math.abs(audio.currentTime - snapped) > 0.25) {
          audio.currentTime = snapped
        }
        finishPendingSeek(audio)
      }
      return
    }
    if (audio.readyState > HTMLMediaElement.HAVE_NOTHING) {
      if (Math.abs(audio.currentTime - pending) > 0.25) {
        audio.currentTime = pending
      }
      finishPendingSeek(audio)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mediaRanges, downloadProgress?.complete, bufferRevision, appendedBytes])

  // Retry discontinuous seek once duration becomes known (Telegram metadata
  // may be missing at attach; seekToTime is a no-op while duration is 0).
  useEffect(() => {
    if (!(duration > 0)) return
    const pending = pendingSeekRef.current
    if (pending === null || discontinuitySeekRef.current) return
    if (downloadProgress?.complete) return
    const audio = audioRef.current
    if (!audio) return
    if (landPlayableTime(pending, mediaBufferedRanges(audio)) !== null) return
    startDiscontinuitySeek()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [duration])

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
