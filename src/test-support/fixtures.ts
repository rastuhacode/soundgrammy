import type { PlaylistsBundle, Track } from '@/types'

export const track = (id: number): Track => ({
  id, tg_user_id: 1, file_id: `f${id}`, file_unique_id: `u${id}`, title: `Track ${id}`, performer: null,
  duration: 120, source: 'mtproto', mime_type: 'audio/mpeg', file_size: 100, created_at: '',
})
export function playlists(trackIds = [1, 2, 3]): PlaylistsBundle {
  return { liked: { id: 10, trackIds: [...trackIds], updatedAt: '' },
    custom: [{ id: 20, name: 'Custom', trackIds: [...trackIds], updatedAt: '' }] }
}
export function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}
