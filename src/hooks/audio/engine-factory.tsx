import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from 'react'
import { appLogger } from '@/lib/app-logger'
import type { AudioEngine } from './engine'
import { TransportEngine } from './transport-engine'
import { HtmlDriver } from './html/html-driver'
import { HtmlEngineHost } from './html/HtmlEngineHost'

export interface AudioEngineComposition {
  engine: AudioEngine
  host?: ReactNode
}
export type AudioEngineFactory = () => AudioEngineComposition

export class AudioEngineCreationError extends Error {
  readonly code = 'creation-failed'
  constructor() {
    super('Could not create the requested audio engine.')
    this.name = 'AudioEngineCreationError'
  }
}

/** A future Rust proxy is selected here; consumers depend only on AudioEngine. */
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
  const driver = new HtmlDriver()
  const engine = new TransportEngine('html', driver, context => appLogger.error({
    source: 'audio', title: 'Audio playback failed',
    description: 'The audio engine could not play the selected track.', context,
  }))
  return { engine, host: <HtmlEngineHost driver={driver} /> }
}

const EngineContext = createContext<AudioEngine | null>(null)

/** Each effect lifetime owns exactly one engine, including Strict Mode replays. */
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
        // Native teardown can be asynchronous. Never create its replacement early.
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
      // Preserve rejection for any replacement, without leaking an unhandled promise on unmount.
      void teardownRef.current.catch(() => {})
    }
  }, [factory])
  if (failure) throw failure
  if (!composition) return null
  return (
    <EngineContext.Provider value={composition.engine}>
      {composition.host}
      {children}
    </EngineContext.Provider>
  )
}

export function useSelectedAudioEngine() {
  const engine = useContext(EngineContext)
  if (!engine) throw new Error('AudioEngineProvider is required')
  return engine
}
