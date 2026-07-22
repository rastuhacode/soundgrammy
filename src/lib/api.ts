import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  AuthOutcome,
  AuthStatus,
  AuthUser,
  CacheChanged,
  CacheSettings,
  CacheUsage,
  CustomPlaylistSummary,
  LikedPlaylist,
  ListenEndReason,
  ListenEndResult,
  PlaylistsBundle,
  QrOutcome,
  SyncResult,
  Track,
  TrackListenStats,
  TrackMetadata,
} from '@/types'

export interface Profile {
  tgUserId: number
  firstName: string
  lastName: string | null
  username: string | null
}

export type TrackSource
  = | { kind: 'cached', path: string }
    | {
      kind: 'stream'
      trackId: number
      mimeType: string
      total: number
    }

// ---- auth ----------------------------------------------------------------

export const api = {
  authStatus: () => invoke<AuthStatus>('auth_status'),
  phoneSendCode: (phone: string) => invoke<void>('phone_send_code', { phone }),
  phoneSignIn: (code: string) => invoke<AuthOutcome>('phone_sign_in', { code }),
  phoneCheckPassword: (password: string) =>
    invoke<AuthUser>('phone_check_password', { password }),
  qrStart: () => invoke<QrOutcome>('qr_start'),
  qrPoll: () => invoke<QrOutcome>('qr_poll'),
  qrCheckPassword: (password: string) =>
    invoke<AuthUser>('qr_check_password', { password }),
  logout: () => invoke<void>('logout'),

  // ---- library ----------------------------------------------------------
  syncSavedMusic: () => invoke<SyncResult>('sync_saved_music'),
  listTracks: () => invoke<Track[]>('list_tracks'),
  getProfile: () => invoke<Profile | null>('get_profile'),
  syncStatus: () => invoke<string | null>('sync_status'),

  // ---- media ------------------------------------------------------------
  getTrackSource: (trackId: number) =>
    invoke<TrackSource>('get_track_source', { trackId }),
  /** Inclusive byte range from an active stream or cached file (chunk-capped). */
  readStreamRange: async (trackId: number, start: number, end: number) => {
    const bytes = await invoke<ArrayBuffer | number[]>('read_stream_range', {
      trackId,
      start,
      end,
    })
    return bytes instanceof ArrayBuffer
      ? new Uint8Array(bytes)
      : Uint8Array.from(bytes)
  },
  /** Prioritize downloading an inclusive byte range (seek-ahead gap fill). */
  ensureStreamRange: (trackId: number, start: number, end: number) =>
    invoke<void>('ensure_stream_range', { trackId, start, end }),
  downloadTrack: (trackId: number) =>
    invoke<string>('download_track', { trackId }),
  prefetchTrack: (trackId: number) =>
    invoke<void>('prefetch_track', { trackId }),
  cacheTrack: (trackId: number) =>
    invoke<void>('cache_track', { trackId }),
  cacheTracks: (trackIds: number[]) =>
    invoke<number[]>('cache_tracks', { trackIds }),
  removeTrackFromCache: (trackId: number) =>
    invoke<void>('remove_track_from_cache', { trackId }),
  clearAudioCache: () => invoke<void>('clear_audio_cache'),
  getCacheStatus: () => invoke<number[]>('get_cache_status'),
  getCacheSettings: () => invoke<CacheSettings>('get_cache_settings'),
  setCacheSettings: (input: {
    limitBytes?: number | null
    ttlSecs?: number | null
  }) =>
    invoke<CacheSettings>('set_cache_settings', {
      limitBytes: input.limitBytes ?? null,
      ttlSecs: input.ttlSecs ?? null,
    }),
  getCacheUsage: () => invoke<CacheUsage>('get_cache_usage'),
  exportTrack: (trackId: number) =>
    invoke<string>('export_track', { trackId }),
  exportTracks: (trackIds: number[]) =>
    invoke<string>('export_tracks', { trackIds }),
  getTrackThumbnail: (trackId: number, highQuality = false) =>
    invoke<string | null>('get_track_thumbnail', { trackId, highQuality }),
  getUserAvatar: () => invoke<string | null>('get_user_avatar'),
  trackMetadata: (trackId: number) =>
    invoke<TrackMetadata>('track_metadata', { trackId }),

  // ---- playlists --------------------------------------------------------
  listPlaylists: () => invoke<PlaylistsBundle>('list_playlists'),
  createPlaylist: (input: {
    name: string
    thumbnailData?: string | null
    thumbnailMime?: string | null
  }) =>
    invoke<CustomPlaylistSummary>('create_playlist', {
      name: input.name,
      thumbnailData: input.thumbnailData ?? null,
      thumbnailMime: input.thumbnailMime ?? null,
    }),
  updatePlaylist: (input: {
    playlistId: number
    name?: string | null
    thumbnailData?: string | null
    thumbnailMime?: string | null
    clearThumbnail?: boolean
  }) =>
    invoke<CustomPlaylistSummary>('update_playlist', {
      playlistId: input.playlistId,
      name: input.name ?? null,
      thumbnailData: input.thumbnailData ?? null,
      thumbnailMime: input.thumbnailMime ?? null,
      clearThumbnail: input.clearThumbnail ?? null,
    }),
  deletePlaylist: (playlistId: number) =>
    invoke<void>('delete_playlist', { playlistId }),
  getPlaylistThumbnail: (playlistId: number) =>
    invoke<string | null>('get_playlist_thumbnail', { playlistId }),
  addTrackToPlaylist: (playlistId: number, trackId: number) =>
    invoke<string>('add_track_to_playlist', { playlistId, trackId }),
  addTracksToPlaylist: (playlistId: number, trackIds: number[]) =>
    invoke<string>('add_tracks_to_playlist', { playlistId, trackIds }),
  removeTrackFromPlaylist: (playlistId: number, position: number) =>
    invoke<string>('remove_track_from_playlist', { playlistId, position }),
  reorderPlaylistTracks: (playlistId: number, trackIds: number[]) =>
    invoke<string>('reorder_playlist_tracks', { playlistId, trackIds }),
  toggleLike: (trackId: number) => invoke<LikedPlaylist>('toggle_like', { trackId }),

  // ---- listen statistics ----------------------------------------------
  recordListenStart: (trackId: number) =>
    invoke<void>('record_listen_start', { trackId }),
  recordListenEnd: (input: {
    trackId: number
    listenedMs: number
    durationMs?: number | null
    endReason: ListenEndReason
  }) =>
    invoke<ListenEndResult>('record_listen_end', {
      trackId: input.trackId,
      listenedMs: input.listenedMs,
      durationMs: input.durationMs ?? null,
      endReason: input.endReason,
    }),
  getTrackListenStats: (trackId: number) =>
    invoke<TrackListenStats | null>('get_track_listen_stats', { trackId }),
  listListenStats: () => invoke<TrackListenStats[]>('list_listen_stats'),
  rebuildListenStats: () => invoke<void>('rebuild_listen_stats'),
}

/** Turns an absolute cache path into an `asset:` URL for `<audio>`/`<img>`. */
export function fileSrc(path: string): string {
  return convertFileSrc(path)
}

export function streamSrc(trackId: number): string {
  return convertFileSrc(String(trackId), 'stream')
}

// ---- events --------------------------------------------------------------

export interface SyncProgress {
  done: number
  total: number
}

export interface DownloadProgress {
  trackId: number
  received: number
  total: number
  ranges: Array<{ start: number, end: number }>
  complete: boolean
}

export function onSyncStart(cb: () => void): Promise<UnlistenFn> {
  return listen('sync:start', () => cb())
}

export function onSyncProgress(
  cb: (p: SyncProgress) => void,
): Promise<UnlistenFn> {
  return listen<SyncProgress>('sync:progress', e => cb(e.payload))
}

export function onSyncDone(cb: () => void): Promise<UnlistenFn> {
  return listen('sync:done', () => cb())
}

export function onDownloadProgress(
  cb: (p: DownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<DownloadProgress>('download:progress', e => cb(e.payload))
}

export function onCacheChanged(
  cb: (p: CacheChanged) => void,
): Promise<UnlistenFn> {
  return listen<CacheChanged>('cache:changed', e => cb(e.payload))
}
