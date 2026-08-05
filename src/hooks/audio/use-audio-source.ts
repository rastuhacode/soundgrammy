import {
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from 'react'
import type { Track } from '@/lib/db'
import {
  api,
  fileSrc,
  onDownloadProgress,
  type DownloadProgress,
} from '@/lib/api'
import {
  estimateMpegDurationSeconds,
  resolveMpegPayloadStart,
} from './mp3-frame-sync'
import { attachMseSession, resolveMseMimeType, type MseSession } from './mse-session'

/** Bytes of MPEG payload to probe for Xing/VBRI / CBR duration. */
const MPEG_DURATION_PROBE_BYTES = 8 * 1024

function isMpegMime(mimeType: string): boolean {
  const base = mimeType.split(';')[0]?.trim().toLowerCase() || ''
  return base === 'audio/mpeg' || base === 'audio/mp3'
}

export interface UseAudioSourceOptions {
  audioRef: RefObject<HTMLAudioElement | null>
  track: Track | null
  applyVolume: () => void
  playAudio: (audio: HTMLAudioElement, generation: number) => void
  isPlayingRef: RefObject<boolean>
  pendingSeekRef: RefObject<number | null>
  resumeAfterSeekRef: RefObject<boolean>
  loadGenerationRef: RefObject<number>
  loadedTrackIdRef: RefObject<number | null>
  sourceErrorRef: RefObject<boolean>
  resetSeekRefs: () => void
  setCurrentTime: Dispatch<SetStateAction<number>>
  setDuration: Dispatch<SetStateAction<number>>
  setPlaying: (playing: boolean) => void
}

export function useAudioSource(options: UseAudioSourceOptions) {
  const {
    audioRef,
    track,
    applyVolume,
    playAudio,
    isPlayingRef,
    pendingSeekRef,
    resumeAfterSeekRef,
    loadGenerationRef,
    loadedTrackIdRef,
    sourceErrorRef,
    resetSeekRefs,
    setCurrentTime,
    setDuration,
    setPlaying,
  } = options

  const [downloadProgress, setDownloadProgress]
    = useState<DownloadProgress | null>(null)
  const [appendedBytes, setAppendedBytes] = useState(0)
  const [bufferRevision, setBufferRevision] = useState(0)
  const [showInitialLoading, setShowInitialLoading] = useState(false)
  /** True once we know this track uses MSE — drives honest buffer chrome. */
  const [streamingMse, setStreamingMse] = useState(false)
  const [sourceLoadEpoch, setSourceLoadEpoch] = useState(0)
  const mseSessionRef = useRef<MseSession | null>(null)

  // Resolve a complete local file or attach an MSE-backed stream.
  useEffect(() => {
    const audio = audioRef.current
    const generation = ++loadGenerationRef.current
    let disposed = false
    let unlisten: (() => void) | undefined
    let mseSession: MseSession | null = null

    loadedTrackIdRef.current = null
    sourceErrorRef.current = false
    mseSessionRef.current = null
    // Reset MSE chrome flag synchronously on track change / remount.
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Must clear before initializeSource races the microtask queue.
    setStreamingMse(false)

    queueMicrotask(() => {
      if (loadGenerationRef.current !== generation) return
      setCurrentTime(0)
      setDuration(track?.duration ?? 0)
      setDownloadProgress(null)
      setAppendedBytes(0)
      setBufferRevision(0)
      // Do not reset streamingMse here — initializeSource sets it true
      // synchronously before its first await; this microtask runs after that
      // and was clearing the flag so the first download:progress painted
      // download-mapped buffer chrome again.
      setShowInitialLoading(Boolean(audio && track))
      resetSeekRefs()
    })

    resetSeekRefs()
    if (!audio) return

    audio.pause()
    audio.removeAttribute('src')
    audio.load()

    if (!track) return

    const finishAttach = (reload = true) => {
      loadedTrackIdRef.current = track.id
      applyVolume()
      // MSE already assigned audio.src to the MediaSource object URL.
      // Calling load() here races sourceopen / addSourceBuffer on WKWebView.
      if (reload) audio.load()
      if (isPlayingRef.current) {
        if (pendingSeekRef.current === null) {
          playAudio(audio, generation)
        }
        else {
          resumeAfterSeekRef.current = true
        }
      }
    }

    const attachCached = (path: string, total: number) => {
      setStreamingMse(false)
      setDownloadProgress({
        trackId: track.id,
        received: total,
        total,
        ranges: [{ start: 0, end: total }],
        complete: true,
      })
      setAppendedBytes(total)
      setShowInitialLoading(false)
      audio.src = fileSrc(path)
      finishAttach()
    }

    /** Full-file download when MSE cannot play (or duration is unknown). */
    const attachAfterFullDownload = async (totalHint: number) => {
      // Paint download-mapped buffer chrome — leave MSE mode so progress is visible.
      setStreamingMse(false)
      setShowInitialLoading(true)
      const path = await api.downloadTrack(track.id)
      if (disposed || loadGenerationRef.current !== generation) return
      const total = track.file_size ?? totalHint
      attachCached(path, total)
    }

    const initializeSource = async () => {
      try {
        // Optimistic: treat as MSE until getTrackSource proves cached.
        // Otherwise download:progress during the await paints fake chrome.
        setStreamingMse(true)

        const stop = await onDownloadProgress((progress) => {
          if (
            disposed
            || loadGenerationRef.current !== generation
            || progress.trackId !== track.id
          ) {
            return
          }
          setDownloadProgress(progress)
          mseSession?.notifyProgress(progress)
          if (progress.received > 0) {
            setShowInitialLoading(false)
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
          attachCached(source.path, total)
          return
        }

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

        const mimeType = resolveMseMimeType(source.mimeType || 'audio/mpeg')
        if (!mimeType) {
          // Do not fall back to progressive stream: — wait for a full cache file.
          await attachAfterFullDownload(source.total)
          return
        }

        // Telegram duration can be 0/missing. MSE audio/mpeg often reports
        // Infinity until EOS, so the progress bar never gets a real length.
        let duration = track.duration ?? 0
        if (!(duration > 0)) {
          let estimated: number | null = null
          if (isMpegMime(mimeType) && source.total > 10) {
            try {
              const headerEnd = Math.min(source.total - 1, 9)
              await api.ensureStreamRange(source.trackId, 0, headerEnd)
              if (disposed || loadGenerationRef.current !== generation) return
              const header = await api.readStreamRange(
                source.trackId,
                0,
                headerEnd,
              )
              if (disposed || loadGenerationRef.current !== generation) return

              const payloadStart = resolveMpegPayloadStart(header, source.total)
              const probeEnd = Math.min(
                source.total - 1,
                payloadStart + MPEG_DURATION_PROBE_BYTES - 1,
              )
              if (probeEnd >= payloadStart) {
                await api.ensureStreamRange(
                  source.trackId,
                  payloadStart,
                  probeEnd,
                )
                if (disposed || loadGenerationRef.current !== generation) return
                const payloadProbe = await api.readStreamRange(
                  source.trackId,
                  payloadStart,
                  probeEnd,
                )
                if (disposed || loadGenerationRef.current !== generation) return
                estimated = estimateMpegDurationSeconds({
                  fileTotal: source.total,
                  payloadStart,
                  payloadProbe,
                })
              }
            }
            catch {
              estimated = null
            }
          }

          if (estimated != null && estimated > 0) {
            duration = estimated
            setDuration(estimated)
          }
          else {
            // Last resort: full download so AVFoundation can report duration.
            await attachAfterFullDownload(source.total)
            return
          }
        }

        mseSession = attachMseSession({
          audio,
          trackId: source.trackId,
          mimeType,
          total: source.total,
          duration,
          onAppendedOffset: (offset) => {
            if (
              !disposed
              && loadGenerationRef.current === generation
            ) {
              setAppendedBytes(offset)
              if (offset > 0) setShowInitialLoading(false)
            }
          },
          onBufferedChanged: () => {
            if (
              !disposed
              && loadGenerationRef.current === generation
            ) {
              setBufferRevision(value => value + 1)
            }
          },
          onError: (failure) => {
            if (
              !disposed
              && loadGenerationRef.current === generation
            ) {
              sourceErrorRef.current = true
              console.error('[audio] MSE session failed', {
                trackId: track.id,
                generation,
                failure,
                mediaSource: {
                  currentSrc: audio.currentSrc,
                  networkState: audio.networkState,
                  readyState: audio.readyState,
                  errorCode: audio.error?.code ?? null,
                  errorMessage: audio.error?.message ?? null,
                },
              })
              setShowInitialLoading(false)
              setPlaying(false)
            }
          },
        })
        mseSessionRef.current = mseSession
        finishAttach(false)
      }
      catch (error) {
        if (!disposed && loadGenerationRef.current === generation) {
          sourceErrorRef.current = true
          console.error('[audio] source initialization failed', {
            trackId: track.id,
            generation,
            error,
          })
          setShowInitialLoading(false)
          setPlaying(false)
        }
      }
    }
    initializeSource()

    return () => {
      disposed = true
      unlisten?.()
      mseSession?.dispose()
      mseSession = null
      mseSessionRef.current = null
      setStreamingMse(false)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sourceLoadEpoch, track?.id])

  const retrySource = () => {
    setSourceLoadEpoch(epoch => epoch + 1)
  }

  const seekMseToTime = (time: number) => {
    const session = mseSessionRef.current
    if (!session) return Promise.resolve()
    return session.seekToTime(time)
  }

  const mseSnapToBufferedTime = (time: number) => {
    const session = mseSessionRef.current
    if (!session) return null
    return session.snapToBufferedTime(time)
  }

  const mseLandToBufferedTime = (time: number) => {
    const session = mseSessionRef.current
    if (!session) return null
    return session.landToBufferedTime(time)
  }

  const isMseActive = () => mseSessionRef.current !== null

  return {
    downloadProgress,
    appendedBytes,
    bufferRevision,
    seekMseToTime,
    mseSnapToBufferedTime,
    mseLandToBufferedTime,
    isMseActive,
    retrySource,
    streamingMse,
    showInitialLoading,
    setShowInitialLoading,
  }
}
