// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { SettingsDialog } from './SettingsDialog'
import { useConnectivityStore } from '@/stores/connectivity-store'

vi.mock('./sections/AudioCacheSection', () => ({ AudioCacheSection: () => null }))
vi.mock('./sections/DiagnosticsSection', () => ({ DiagnosticsSection: () => null }))
vi.mock('./sections/FullscreenPlayerSection', () => ({ FullscreenPlayerSection: () => null }))
vi.mock('./sections/IntegrationsSection', () => ({ IntegrationsSection: () => null }))
vi.mock('./sections/ListeningStatisticsSection', () => ({ ListeningStatisticsSection: () => null }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
let root: ReturnType<typeof createRoot>
let host: HTMLDivElement
let request: ReturnType<typeof vi.spyOn>

beforeEach(async () => {
  useConnectivityStore.getState().reset()
  request = vi.spyOn(useConnectivityStore.getState(), 'requestSync').mockResolvedValue(undefined)
  host = document.createElement('div')
  document.body.append(host)
  root = createRoot(host)
  await act(async () => root.render(createElement(SettingsDialog, { open: true, onOpenChange: vi.fn() })))
})
afterEach(async () => {
  await act(async () => root.unmount())
  host.remove()
  request.mockRestore()
})
const button = (text: string) => Array.from(document.body.querySelectorAll<HTMLButtonElement>('button')).find(b => b.textContent === text)!
async function click(text: string) {
  expect(button(text)).toBeDefined()
  await act(async () => button(text).click())
}

it('requires confirmation in Settings before starting full reindexing', async () => {
  await click('Music library')
  await click('Fully reindex tracks')
  expect(document.body.textContent).toContain('Fully reindex your library?')
  expect(request).not.toHaveBeenCalled()
  await click('Cancel')
  expect(request).not.toHaveBeenCalled()
  await click('Fully reindex tracks')
  await click('Confirm full reindex')
  expect(request).toHaveBeenCalledExactlyOnceWith('full')
})

it('disables full reindexing while another sync is active', async () => {
  await click('Music library')
  await act(async () => useConnectivityStore.setState({ syncing: true }))
  expect(button('Fully reindex tracks').disabled).toBe(true)
  expect(request).not.toHaveBeenCalled()
})
