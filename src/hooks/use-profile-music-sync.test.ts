// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { expect, it, vi } from 'vitest'
import { useProfileMusicSync } from './use-profile-music-sync'
import { useConnectivityStore } from '@/stores/connectivity-store'
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

it('routes the tower action to soft sync and exposes its progress in the menu', async () => {
  useConnectivityStore.getState().reset()
  useConnectivityStore.setState({ phase: 'online', syncing: true, blockingIndex: false, progress: { done: 10037, total: 10038 } })
  const request = vi.spyOn(useConnectivityStore.getState(), 'requestSync').mockResolvedValue(undefined)
  let model!: ReturnType<typeof useProfileMusicSync>
  function Probe() {
    model = useProfileMusicSync()
    return null
  }
  const root = createRoot(document.createElement('div'))
  try {
    await act(async () => root.render(createElement(Probe)))
    expect(model.statusDetail).toBe('Syncing 10037 of 10038 tracks…')
    await act(async () => model.requestSync())
    expect(request).toHaveBeenCalledExactlyOnceWith('soft')
    await act(async () => useConnectivityStore.setState({ blockingIndex: true }))
    expect(model.statusDetail).toBe('Indexing 10037 of 10038 tracks…')
  }
  finally {
    await act(async () => root.unmount())
    request.mockRestore()
  }
})
