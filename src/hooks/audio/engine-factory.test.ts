import { afterEach, describe, expect, it, vi } from 'vitest'
import { createAudioEngine, AudioEngineCreationError } from './engine-factory'
import { NativeRustAudioEngine } from './native-engine'

vi.mock('./native-engine', () => ({
  NativeRustAudioEngine: vi.fn(class {
    readonly kind = 'native-rust'
  }),
}))

afterEach(() => {
  vi.unstubAllEnvs()
  vi.clearAllMocks()
})

describe('default playback engine', () => {
  it.each([true, false])('uses native playback with DEV=%s and no selection flag', (dev) => {
    vi.stubEnv('DEV', dev)
    vi.stubEnv('VITE_AUDIO_ENGINE', undefined)
    const composition = createAudioEngine()
    expect(composition.engine).toBeInstanceOf(NativeRustAudioEngine)
    expect(Object.keys(composition)).toEqual(['engine'])
    expect(NativeRustAudioEngine).toHaveBeenCalledTimes(1)
  })

  it('does not fall back when an injected factory fails', () => {
    expect(() => createAudioEngine(() => {
      throw new Error('failed')
    })).toThrow(AudioEngineCreationError)
    expect(NativeRustAudioEngine).not.toHaveBeenCalled()
  })
})
