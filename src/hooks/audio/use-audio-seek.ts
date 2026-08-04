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
  /** True while an MSE session owns the element (even after download completes). */
  isMseActive: () => boolean
  /**
   * React state: true once getTrackSource returns stream. Prefer this over
   * isMseActive() for buffer chrome — the ref lags the first download:progress.
   */
  streamingMse?: boolean
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
    isMseActive,
    streamingMse = false,
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
    () => {
      // MSE: only paint honest SourceBuffer ranges. Download-mapped
      // cachedRanges / byte-proportional playableEnd overstate coverage
      // (especially past ID3), then shrink when mediaRanges appear — the
      // "buffer clears then restarts" flash on every uncached stream.
      // Use streamingMse (state) not isMseActive() (ref) — progress can arrive
      // before the session ref is set.
      if (streamingMse || isMseActive()) {
        return computeBufferedRanges({
          mediaRanges,
          playableEnd: 0,
          duration,
          currentTime,
          cachedRanges: [],
        })
      }
      return computeBufferedRanges({
        mediaRanges,
        playableEnd,
        duration,
        currentTime,
        cachedRanges,
        fullyCached: downloadProgress?.complete,
      })
    },
    [cachedRanges, currentTime, downloadProgress?.complete, duration, isMseActive, mediaRanges, playableEnd, streamingMse],
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
    // Cached asset: any time is fine once complete. MSE: only SourceBuffer.
    if (isMseActive() || !downloadProgress?.complete) {
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
          let snapped: number | null = null
          try {
            snapped = landSeekTime(pending, mediaBufferedRanges(audio))
          }
          catch {
            return
          }

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
      catch {
        // Discontinuity failed — leave pending seek for canplay / next scrub.
      }
      finally {
        discontinuitySeekRef.current = false
      }
    })()
  }

  /** Scrub playable check — tight snap so distant stale islands are not playable. */
  const landPlayableTime = (time: number, ranges: TimeRange[]): number | null => {
    // MSE owns the timeline even after the file has finished downloading —
    // `complete` only means bytes are on disk, not that SourceBuffer has them.
    if (isMseActive()) {
      return mseSnapToBufferedTime?.(time) ?? null
    }
    if (downloadProgress?.complete) return time
    return snapTimeToRanges(time, ranges)
  }

  /** Finish/land after discontinuity — SourceBuffer island, not element buffered. */
  const landSeekTime = (time: number, ranges: TimeRange[]): number | null => {
    if (isMseActive()) {
      return mseLandToBufferedTime?.(time) ?? null
    }
    if (downloadProgress?.complete) return time
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
        try {
          audio.currentTime = landed
        }
        catch {
          pendingSeekRef.current = time
          startDiscontinuitySeek()
        }
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
    if (isMseActive() || !downloadProgress?.complete) {
      const snapped = landSeekTime(pendingSeek, mediaBufferedRanges(audio))
      if (snapped === null) return
      try {
        audio.currentTime = snapped
      }
      catch {
        // canplay / discontinuity will retry
      }
      return
    }
    try {
      audio.currentTime = pendingSeek
    }
    catch {
      // ignore
    }
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
    if (isMseActive() || !downloadProgress?.complete) {
      const snapped = landSeekTime(pending, mediaRanges)
      if (snapped === null) return
      if (audio.readyState > HTMLMediaElement.HAVE_NOTHING) {
        if (Math.abs(audio.currentTime - snapped) > 0.25) {
          try {
            audio.currentTime = snapped
          }
          catch {
            return
          }
        }
        finishPendingSeek(audio)
      }
      return
    }
    if (audio.readyState > HTMLMediaElement.HAVE_NOTHING) {
      if (Math.abs(audio.currentTime - pending) > 0.25) {
        try {
          audio.currentTime = pending
        }
        catch {
          return
        }
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
    if (!isMseActive() && downloadProgress?.complete) return
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
