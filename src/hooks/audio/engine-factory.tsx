import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from 'react'
import { NativeRustAudioEngine } from './native-engine'
import type { AudioEngine } from './engine'

export interface AudioEngineComposition {
  engine: AudioEngine
}
export type AudioEngineFactory = () => AudioEngineComposition

export class AudioEngineCreationError extends Error {
  readonly code = 'creation-failed'
  constructor() {
    super('Could not create the requested audio engine.')
    this.name = 'AudioEngineCreationError'
  }
}

/** Desktop playback always uses the native Rust service. */
export function createAudioEngine(override?: AudioEngineFactory): AudioEngineComposition {
  if (override) {
    try {
      return override()
    }
    catch {
      // Do not auto-fallback: a failed external factory may own native resources.
      throw new AudioEngineCreationError()
    }
  }
  return { engine: new NativeRustAudioEngine() }
}

const EngineContext = createContext<AudioEngine | null>(null)

/** Each effect owns one adapter; native playback outlives the view. */
export function AudioEngineProvider({ children, factory }: {
  children?: ReactNode
  factory?: AudioEngineFactory
}) {
  const [composition, setComposition] = useState<AudioEngineComposition | null>(null)
  const [failure, setFailure] = useState<AudioEngineCreationError | null>(null)
  const teardownRef = useRef<Promise<void>>(Promise.resolve())
  useEffect(() => {
    let cancelled = false
    let next: AudioEngineComposition | null = null
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Detach old consumers before replacing their resource.
    setComposition(null)
    void (async () => {
      try {
        // Finish detaching old subscriptions before attaching a replacement.
        await teardownRef.current
        if (cancelled) return
        next = createAudioEngine(factory)
        setFailure(null)
        setComposition(next)
      }
      catch {
        if (!cancelled) setFailure(new AudioEngineCreationError())
      }
    })()
    return () => {
      cancelled = true
      if (!next) return
      teardownRef.current = next.engine.destroy()
      // Preserve cleanup failures for a replacement without leaking a rejection on unmount.
      void teardownRef.current.catch(() => {})
    }
  }, [factory])
  if (failure) throw failure
  if (!composition) return null
  return (
    <EngineContext.Provider value={composition.engine}>
      {children}
    </EngineContext.Provider>
  )
}

export function useSelectedAudioEngine() {
  const engine = useContext(EngineContext)
  if (!engine) throw new Error('AudioEngineProvider is required')
  return engine
}
