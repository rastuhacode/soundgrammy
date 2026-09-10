import { useEffect, useRef } from 'react'
import { useLocalStorage } from '@mantine/hooks'
import { normalizeVolume, parseStoredVolume, VOLUME_DEFAULT } from '@/lib/volume'
import type { AudioEngine } from './engine'

/** Persistence and remembered pre-mute volume are UI preferences, not transport. */
export function useAudioVolume(engine: AudioEngine) {
  const [volume, setStoredVolume] = useLocalStorage<number>({
    key: 'soundgrammy-volume', defaultValue: VOLUME_DEFAULT,
    getInitialValueInEffect: false, deserialize: parseStoredVolume,
  })
  const volumeRef = useRef(volume)
  const preMuteVolumeRef = useRef(volume > 0 ? volume : VOLUME_DEFAULT)
  const handleVolumeChange = (value: number) => {
    const next = normalizeVolume(value)
    if (next > 0) preMuteVolumeRef.current = next
    volumeRef.current = next
    setStoredVolume(next)
    void engine.setVolume(next)
  }
  const handleMuteToggle = () => {
    handleVolumeChange(volumeRef.current === 0 ? preMuteVolumeRef.current : 0)
  }
  useEffect(() => {
    volumeRef.current = volume
    if (volume > 0) preMuteVolumeRef.current = volume
    void engine.setVolume(volume)
  }, [engine, volume])
  return { volume, handleVolumeChange, handleMuteToggle }
}
