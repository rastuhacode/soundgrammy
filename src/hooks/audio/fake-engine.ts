import type { AudioTrackRequest } from './engine'
import { TransportEngine, type TransportDriver, type TransportObserver, type TransportSession } from './transport-engine'

/** Deterministic test transport. Retained observers allow explicit stale-callback tests. */
export class FakeAudioDriver implements TransportDriver {
  readonly sessions: Array<{ request: AudioTrackRequest, observer: TransportObserver, disposed: boolean }> = []
  open(request: AudioTrackRequest, observer: TransportObserver): TransportSession {
    const session = { request, observer, disposed: false }
    this.sessions.push(session)
    let playing = false
    let ready = false
    queueMicrotask(() => {
      if (session.disposed) return
      ready = true
      observer.state({ status: playing ? 'playing' : 'ready', initialLoading: false })
    })
    return {
      play: () => {
        playing = true
        if (ready) observer.state({
          status: 'playing',
        })
      },
      pause: () => {
        playing = false
        observer.state({ status: 'paused' })
      },
      seek: seconds => observer.state({ currentTimeSeconds: seconds }),
      beginSeek: () => {}, endSeek: () => {}, setVolume: () => {},
      dispose: () => {
        session.disposed = true
      },
    }
  }
}
export function createFakeAudioEngine() {
  const driver = new FakeAudioDriver()
  return { engine: new TransportEngine('test', driver), driver }
}
