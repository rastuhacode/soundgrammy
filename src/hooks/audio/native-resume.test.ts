// @vitest-environment jsdom
import { expect, it, vi } from 'vitest'
import { NativeRustAudioEngine, type NativeTransport } from './native-engine'
import { api } from '@/lib/api'
import { applyPlaybackSession, attachPlayer, usePlayerStore } from '@/stores/player-store'
import { useRepeatStore } from '@/stores/repeat-store'

vi.mock('@/lib/api', () => ({ api: { nativePlayerCommand: vi.fn() }, onNativeAudioState: vi.fn(), onNativeAudioEvent: vi.fn() }))
const initial = { revision: 1, kind: 'native-rust', status: 'playing', trackId: 1, attemptId: 'native:1', currentTimeSeconds: 5,
  durationSeconds: 120, bufferedRanges: [], volumePercent: 50, initialLoading: false, error: null }

it('resynchronizes on visibility/pageshow and removes all listeners on detach', async () => {
  const unlistenState = vi.fn()
  const unlistenEvent = vi.fn()
  const transport: NativeTransport = {
    state: vi.fn(async () => unlistenState), event: vi.fn(async () => unlistenEvent), snapshot: vi.fn(async () => initial),
    load: vi.fn(), unload: vi.fn(), play: vi.fn(), pause: vi.fn(), seek: vi.fn(), volume: vi.fn(),
  }
  const engine = new NativeRustAudioEngine(transport)
  await engine.ready()
  vi.mocked(transport.snapshot).mockResolvedValue({ ...initial, revision: 20, attemptId: 'native:4', trackId: 4, currentTimeSeconds: 35 })
  const visibility = vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden')
  document.dispatchEvent(new Event('visibilitychange'))
  expect(transport.snapshot).toHaveBeenCalledTimes(1)
  visibility.mockReturnValue('visible')
  document.dispatchEvent(new Event('visibilitychange'))
  await vi.waitFor(() => expect(engine.getSnapshot().trackId).toBe(4))
  expect(engine.getSnapshot().currentTimeSeconds).toBe(35)
  window.dispatchEvent(new Event('pageshow'))
  await vi.waitFor(() => expect(transport.snapshot).toHaveBeenCalledTimes(3))
  await engine.destroy()
  window.dispatchEvent(new Event('pageshow'))
  document.dispatchEvent(new Event('visibilitychange'))
  expect(transport.snapshot).toHaveBeenCalledTimes(3)
  expect(unlistenState).toHaveBeenCalledOnce()
  expect(unlistenEvent).toHaveBeenCalledOnce()
  expect(transport.unload).not.toHaveBeenCalled()
  visibility.mockRestore()
})

it('offers legacy preferences without overwriting the live mirror during attachment', async () => {
  localStorage.setItem('soundgrammy-repeat', JSON.stringify('all'))
  localStorage.setItem('soundgrammy-shuffle', JSON.stringify('on'))
  localStorage.setItem('soundgrammy-shuffle-mode', JSON.stringify('smart'))
  usePlayerStore.setState({ nativeRevision: -1 })
  applyPlaybackSession({ revision: 5, attempt: 2, isPlaying: false, endReason: '',
    queue: { tracks: [], cursor: -1, source: null, sourceIndices: null, baseEntries: null },
    preferences: { repeat: 'one', shuffle: 'off', mode: 'random' } })
  await attachPlayer()
  expect(api.nativePlayerCommand).toHaveBeenCalledWith({ type: 'attach', preferences: { repeat: 'all', shuffle: 'on', mode: 'smart' } })
  expect(useRepeatStore.getState().repeat).toBe('one')
  localStorage.clear()
})
