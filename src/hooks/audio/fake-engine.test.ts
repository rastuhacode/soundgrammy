import { audioEngineContract } from './engine.contract'
import { createFakeAudioEngine } from './fake-engine'

audioEngineContract('fake', async () => {
  const { engine, driver } = createFakeAudioEngine()
  return {
    engine,
    settle: async () => {
      await Promise.resolve()
    },
    status: (index, status) => driver.sessions[index]!.observer.state({ status }),
    ended: index => driver.sessions[index]!.observer.ended(),
    fail: index => driver.sessions[index]!.observer.failed({ code: 'decode-failed', message: 'Decode failed', recoverable: true }),
    cleanup: () => engine.destroy(),
  }
})
