import { useEffect, useRef, useState, type RefObject } from 'react'
import { useLocalStorage } from '@mantine/hooks'
import { z } from 'zod'

const VOLUME_STORAGE_KEY = 'soundgrammy-volume'
const VOLUME_DEFAULT = 25 // 25% (player shouldn't scream by default)

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

export function useAudioVolume(
  audioRef: RefObject<HTMLAudioElement | null>,
) {
  const [volume, setVolume] = useLocalStorage<number>({
    key: VOLUME_STORAGE_KEY,
    defaultValue: VOLUME_DEFAULT,
    getInitialValueInEffect: false,
    deserialize: parseStoredVolume,
  })
  const [isMuted, setIsMuted] = useState(false)
  const volumeRef = useRef(volume)
  const preMuteVolumeRef = useRef(volume)

  const applyVolume = () => {
    const audio = audioRef.current
    if (!audio) return
    audio.volume = volumeRef.current / 100
  }

  const audioRefCallback = (node: HTMLAudioElement | null) => {
    audioRef.current = node
    if (node) {
      node.volume = volumeRef.current / 100
    }
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

  useEffect(() => {
    volumeRef.current = volume
    const audio = audioRef.current
    if (!audio) return
    audio.volume = volumeRef.current / 100
  }, [audioRef, volume])

  return {
    volume,
    applyVolume,
    audioRefCallback,
    handleVolumeChange,
    handleMuteToggle,
  }
}
