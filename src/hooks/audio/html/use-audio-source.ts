import {
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from 'react'
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
let playbackSessionSequence = 0

function createPlaybackSessionId(trackId: number): string {
  playbackSessionSequence += 1
  const uuid = globalThis.crypto?.randomUUID?.()
  return uuid ?? `${trackId}-${Date.now()}-${playbackSessionSequence}`
}

function isMpegMime(mimeType: string): boolean {
  const base = mimeType.split(';')[0]?.trim().toLowerCase() || ''
  return base === 'audio/mpeg' || base === 'audio/mp3'
}

export interface UseAudioSourceOptions {
  audioRef: RefObject<HTMLAudioElement | null>
  track: { id: number, duration: number | null } | null
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
  const mseSessionRef = useRef<MseSession | null>(null)
  const disposeRef = useRef<() => void>(() => {})

  // Resolve a complete local file or attach an MSE-backed stream.
  useEffect(() => {
    const audio = audioRef.current
    const generation = ++loadGenerationRef.current
    const sessionId = track
      ? createPlaybackSessionId(track.id)
      : `idle-${generation}`
    let disposed = false
    let unlisten: (() => void) | undefined
    let mseSession: MseSession | null = null
    let backendSessionMayExist = false
    let latestStreamProgress: DownloadProgress | null = null

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
      // Keep the loading indicator until the full file is available.
      setStreamingMse(false)
      setShowInitialLoading(true)
      const path = await api.downloadTrackForPlayback(track.id, sessionId)
      if (disposed || loadGenerationRef.current !== generation) return
      const total = totalHint
      attachCached(path, total)
    }

    const initializeSource = async () => {
      try {
        // Optimistic: treat as MSE until getTrackSource proves cached.
        // Until source resolution, downloaded bytes are not playable ranges.
        setStreamingMse(true)

        const stop = await onDownloadProgress((progress) => {
          if (
            disposed
            || loadGenerationRef.current !== generation
            || progress.trackId !== track.id
          ) {
            return
          }
          latestStreamProgress = progress
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

        backendSessionMayExist = true
        const source = await api.getTrackSource(track.id, sessionId)
        if (disposed || loadGenerationRef.current !== generation) {
          // The backend may finish opening after our earlier close request.
          if (source.kind === 'stream') void api.closeStreamSession(sessionId).catch(() => {})
          return
        }

        if (source.kind === 'cached') {
          backendSessionMayExist = false
          const total = 1
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
              await api.ensureStreamRange(
                source.trackId,
                sessionId,
                0,
                headerEnd,
              )
              if (disposed || loadGenerationRef.current !== generation) return
              const header = await api.readStreamRange(
                source.trackId,
                sessionId,
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
                  sessionId,
                  payloadStart,
                  probeEnd,
                )
                if (disposed || loadGenerationRef.current !== generation) return
                const payloadProbe = await api.readStreamRange(
                  source.trackId,
                  sessionId,
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
          sessionId,
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
          onError: () => {
            if (
              !disposed
              && loadGenerationRef.current === generation
            ) {
              sourceErrorRef.current = true

              setShowInitialLoading(false)
              setPlaying(false)
            }
          },
        })
        mseSessionRef.current = mseSession
        if (latestStreamProgress) {
          mseSession.notifyProgress(latestStreamProgress)
        }
        finishAttach(false)
      }
      catch {
        if (!disposed && loadGenerationRef.current === generation) {
          sourceErrorRef.current = true

          setShowInitialLoading(false)
          setPlaying(false)
        }
      }
    }
    initializeSource()

    const dispose = () => {
      if (disposed) return
      disposed = true
      ++loadGenerationRef.current
      loadedTrackIdRef.current = null
      audio.pause()
      audio.removeAttribute('src')
      audio.load()
      unlisten?.()
      mseSession?.dispose()
      if (backendSessionMayExist) {
        void api.closeStreamSession(sessionId).catch(() => {})
      }
      mseSession = null
      mseSessionRef.current = null
    }
    disposeRef.current = dispose
    window.addEventListener('pagehide', dispose)
    window.addEventListener('beforeunload', dispose)
    return () => {
      window.removeEventListener('pagehide', dispose)
      window.removeEventListener('beforeunload', dispose)
      dispose()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [track?.id])

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
    disposeSource: () => disposeRef.current(),
    downloadProgress,
    appendedBytes,
    bufferRevision,
    seekMseToTime,
    mseSnapToBufferedTime,
    mseLandToBufferedTime,
    isMseActive,
    streamingMse,
    showInitialLoading,
    setShowInitialLoading,
  }
}
