import {
  useEffect,
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
  streamSrc,
  type DownloadProgress,
} from '@/lib/api'

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
    resetSeekRefs,
    setCurrentTime,
    setDuration,
    setPlaying,
  } = options

  const [downloadProgress, setDownloadProgress]
    = useState<DownloadProgress | null>(null)
  const [showInitialLoading, setShowInitialLoading] = useState(false)

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
      resetSeekRefs()
    })

    resetSeekRefs()
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

  return {
    downloadProgress,
    showInitialLoading,
    setShowInitialLoading,
  }
}
