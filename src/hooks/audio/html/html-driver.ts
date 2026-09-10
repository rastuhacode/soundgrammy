import type { AudioTrackRequest } from '../engine'
import type { TransportDriver, TransportObserver, TransportSession } from '../transport-engine'

export interface HtmlControls {
  play(): void
  pause(): void
  seek(seconds: number): void
  beginSeek(): void
  endSeek(): void
  volume(percent: number): void
  dispose(): void
}

/** Mailbox for commands arriving before the React host has mounted. */
export class HtmlSession implements TransportSession {
  private controls: HtmlControls | null = null
  private stopped = false
  private playing = false
  private volume = 100
  private position: number | null = null
  private scrubbing = false

  constructor(readonly request: AudioTrackRequest, readonly observer: TransportObserver) {}

  attach(controls: HtmlControls) {
    if (this.stopped) {
      controls.dispose()
      return () => {}
    }
    this.controls = controls
    controls.volume(this.volume)
    if (this.scrubbing) controls.beginSeek()
    if (this.position !== null) controls.seek(this.position)
    if (this.playing) controls.play()
    return () => {
      if (this.controls === controls) this.controls = null
      controls.dispose()
    }
  }

  play() {
    this.playing = true
    this.controls?.play()
  }

  pause() {
    this.playing = false
    this.controls?.pause()
  }

  seek(seconds: number) {
    this.position = seconds
    this.controls?.seek(seconds)
  }

  beginSeek() {
    this.scrubbing = true
    this.controls?.beginSeek()
  }

  endSeek() {
    this.scrubbing = false
    this.controls?.endSeek()
  }

  setVolume(percent: number) {
    this.volume = percent
    this.controls?.volume(percent)
  }

  dispose() {
    if (this.stopped) return
    this.stopped = true
    this.controls?.dispose()
    this.controls = null
  }
}

/** Rendering is private to the factory; the neutral engine never knows React. */
export class HtmlDriver implements TransportDriver {
  private current: { session: HtmlSession, key: number } | null = null
  private sequence = 0
  private listeners = new Set<() => void>()
  getSnapshot = () => this.current
  subscribe = (listener: () => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  open(request: AudioTrackRequest, observer: TransportObserver): TransportSession {
    const session = new HtmlSession(request, observer)
    this.current = { session, key: ++this.sequence }
    this.listeners.forEach(listener => listener())
    return {
      play: () => session.play(), pause: () => session.pause(),
      seek: seconds => session.seek(seconds),
      beginSeek: () => session.beginSeek(), endSeek: () => session.endSeek(),
      setVolume: percent => session.setVolume(percent),
      dispose: () => {
        session.dispose()
        if (this.current?.session !== session) return
        this.current = null
        this.listeners.forEach(listener => listener())
      },
    }
  }
}
