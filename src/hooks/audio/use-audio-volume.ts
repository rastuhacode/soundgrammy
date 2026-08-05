import { useEffect, useRef, type RefObject } from 'react'
import { useLocalStorage } from '@mantine/hooks'
import {
  normalizeVolume,
  parseStoredVolume,
  VOLUME_DEFAULT,
} from '@/lib/volume'

const VOLUME_STORAGE_KEY = 'soundgrammy-volume'

export function useAudioVolume(
  audioRef: RefObject<HTMLAudioElement | null>,
) {
  const [volume, setStoredVolume] = useLocalStorage<number>({
    key: VOLUME_STORAGE_KEY,
    defaultValue: VOLUME_DEFAULT,
    getInitialValueInEffect: false,
    deserialize: parseStoredVolume,
  })
  const volumeRef = useRef(volume)
  const preMuteVolumeRef = useRef(volume > 0 ? volume : VOLUME_DEFAULT)

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

  const handleVolumeChange = (value: number) => {
    const nextVolume = normalizeVolume(value)
    if (nextVolume > 0) preMuteVolumeRef.current = nextVolume
    setStoredVolume(nextVolume)
    volumeRef.current = nextVolume
    applyVolume()
  }

  const handleMuteToggle = () => {
    if (volumeRef.current === 0) {
      handleVolumeChange(preMuteVolumeRef.current || VOLUME_DEFAULT)
    }
    else {
      preMuteVolumeRef.current = volumeRef.current
      handleVolumeChange(0)
    }
  }

  useEffect(() => {
    volumeRef.current = volume
    if (volume > 0) preMuteVolumeRef.current = volume
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
