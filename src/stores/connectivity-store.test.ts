import { beforeEach, describe, expect, it } from 'vitest'
import { useConnectivityStore } from './connectivity-store'

describe('useConnectivityStore', () => {
  beforeEach(() => {
    useConnectivityStore.getState().reset()
  })

  it('starts connecting and cycles phases', () => {
    expect(useConnectivityStore.getState().phase).toBe('connecting')
    useConnectivityStore.getState().setOnline()
    expect(useConnectivityStore.getState().phase).toBe('online')
    useConnectivityStore.getState().setOffline()
    expect(useConnectivityStore.getState().phase).toBe('offline')
    useConnectivityStore.getState().setConnecting()
    expect(useConnectivityStore.getState().phase).toBe('connecting')
    useConnectivityStore.getState().reset()
    expect(useConnectivityStore.getState().phase).toBe('connecting')
  })
})
