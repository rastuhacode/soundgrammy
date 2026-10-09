import { appLogger } from '@/lib/app-logger'

/** Own each async registration separately, including partial setup failures. */
export function ownEventListeners(registrations: Promise<() => void>[]): () => void {
  let active = true
  const listeners: (() => void)[] = []
  for (const registration of registrations) {
    void registration.then((unlisten) => {
      if (active) listeners.push(unlisten)
      else unlisten()
    }).catch((error: unknown) => {
      appLogger.error({ source: 'application', title: 'Could not subscribe to backend events', error })
    })
  }
  return () => {
    active = false
    listeners.splice(0).forEach(unlisten => unlisten())
  }
}
