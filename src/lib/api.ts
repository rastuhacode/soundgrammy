import { invoke as tauriInvoke, convertFileSrc } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { appLogger } from '@/lib/app-logger'
import type {
  AuthOutcome,
  AuthStatus,
  AuthUser,
  BounceProfileResponse,
  PhoneSendCodeOutcome,
  CacheChanged,
  CacheSettings,
  CacheTracksProgress,
  CacheUsage,
  CustomPlaylistSummary,
  LikedPlaylist,
  ListenEndReason,
  ListenEndResult,
  LastFmPendingAction,
  LastFmStatus,
  PlaylistDownloadProgress,
  PlaylistDownloadResult,
  PlaylistImportPreview,
  PlaylistImportResult,
  PlaylistRecipeSource,
  PlaylistsBundle,
  ProxySettings,
  ProxySettingsView,
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
  phone: string | null
}

export type TrackSource
  = | { kind: 'cached', path: string }
    | {
      kind: 'stream'
      trackId: number
      sessionId: string
      mimeType: string
      total: number
    }

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(command, args)
  }
  catch (error) {
    // Never include command arguments: auth and proxy commands can carry secrets.
    appLogger.error({
      source: 'backend',
      title: 'Backend command failed',
      description: `The ${command} command returned an error.`,
      error,
      context: { command },
    })
    throw error
  }
}

// ---- auth ----------------------------------------------------------------

export const api = {
  authStatus: () => invoke<AuthStatus>('auth_status'),
  refreshAuth: () => invoke<AuthStatus>('refresh_auth'),
  phoneSendCode: (phone: string) =>
    invoke<PhoneSendCodeOutcome>('phone_send_code', { phone }),
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
  setFullscreenDisplayAwake: (enabled: boolean) =>
    invoke<void>('set_fullscreen_display_awake', { enabled }),

  // ---- media ------------------------------------------------------------
  getTrackSource: (trackId: number, sessionId: string) =>
    invoke<TrackSource>('get_track_source', { trackId, sessionId }),
  /** Inclusive byte range from an active playback stream (chunk-capped). */
  readStreamRange: async (
    trackId: number,
    sessionId: string,
    start: number,
    end: number,
  ) => {
    const bytes = await invoke<ArrayBuffer | number[]>('read_stream_range', {
      trackId,
      sessionId,
      start,
      end,
    })
    return bytes instanceof ArrayBuffer
      ? new Uint8Array(bytes)
      : Uint8Array.from(bytes)
  },
  /** Prioritize downloading an inclusive byte range (seek-ahead gap fill). */
  ensureStreamRange: (
    trackId: number,
    sessionId: string,
    start: number,
    end: number,
  ) => invoke<void>('ensure_stream_range', { trackId, sessionId, start, end }),
  /** Low-priority storage-only fill of the backend-validated ID3v2 prefix. */
  backfillStreamId3: (
    trackId: number,
    sessionId: string,
  ) => invoke<void>('backfill_stream_id3', {
    trackId,
    sessionId,
  }),
  downloadTrackForPlayback: (trackId: number, sessionId: string) =>
    invoke<string>('download_track_for_playback', { trackId, sessionId }),
  closeStreamSession: (sessionId: string) =>
    invoke<void>('close_stream_session', { sessionId }),
  downloadTrack: (trackId: number) =>
    invoke<string>('download_track', { trackId }),
  prefetchTrack: (trackId: number) =>
    invoke<void>('prefetch_track', { trackId }),
  cacheTrack: (trackId: number) =>
    invoke<void>('cache_track', { trackId }),
  cacheTracks: (trackIds: number[], jobId?: string | null) =>
    invoke<number[]>('cache_tracks', {
      trackIds,
      jobId: jobId ?? null,
    }),
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
  getProxySettings: () => invoke<ProxySettingsView>('get_proxy_settings'),
  setProxySettings: (input: {
    enabled: boolean
    server: string
    port: number
    secret: string
  }) =>
    invoke<ProxySettingsView>('set_proxy_settings', {
      enabled: input.enabled,
      server: input.server,
      port: input.port,
      secret: input.secret,
    }),
  parseProxyLink: (link: string) =>
    invoke<ProxySettings>('parse_proxy_link', { link }),
  exportTrack: (trackId: number) =>
    invoke<string>('export_track', { trackId }),
  exportTracks: (trackIds: number[]) =>
    invoke<string>('export_tracks', { trackIds }),
  downloadPlaylist: (name: string, trackIds: number[], jobId: string) =>
    invoke<PlaylistDownloadResult>('download_playlist', {
      name,
      trackIds,
      jobId,
    }),
  getTrackThumbnail: (trackId: number, highQuality = false) =>
    invoke<string | null>('get_track_thumbnail', { trackId, highQuality }),
  getUserAvatar: () => invoke<string | null>('get_user_avatar'),
  trackMetadata: (trackId: number) =>
    invoke<TrackMetadata>('track_metadata', { trackId }),
  getTrackBounceProfile: (trackId: number) =>
    invoke<BounceProfileResponse>('get_track_bounce_profile', { trackId }),

  // ---- playlists --------------------------------------------------------
  listPlaylists: () => invoke<PlaylistsBundle>('list_playlists'),
  createPlaylist: (input: { name: string }) =>
    invoke<CustomPlaylistSummary>('create_playlist', {
      name: input.name,
    }),
  updatePlaylist: (input: {
    playlistId: number
    name?: string | null
  }) =>
    invoke<CustomPlaylistSummary>('update_playlist', {
      playlistId: input.playlistId,
      name: input.name ?? null,
    }),
  deletePlaylist: (playlistId: number) =>
    invoke<void>('delete_playlist', { playlistId }),
  addTrackToPlaylist: (playlistId: number, trackId: number) =>
    invoke<string>('add_track_to_playlist', { playlistId, trackId }),
  addTracksToPlaylist: (playlistId: number, trackIds: number[]) =>
    invoke<string>('add_tracks_to_playlist', { playlistId, trackIds }),
  removeTrackFromPlaylist: (playlistId: number, position: number) =>
    invoke<string>('remove_track_from_playlist', { playlistId, position }),
  reorderPlaylistTracks: (playlistId: number, trackIds: number[]) =>
    invoke<string>('reorder_playlist_tracks', { playlistId, trackIds }),
  toggleLike: (trackId: number) => invoke<LikedPlaylist>('toggle_like', { trackId }),
  exportPlaylistJson: (source: PlaylistRecipeSource, path: string) =>
    invoke<void>('export_playlist_json', { source, path }),
  analyzePlaylistJson: (path: string) =>
    invoke<PlaylistImportPreview>('analyze_playlist_json', { path }),
  importPlaylistJson: (path: string, name?: string | null) =>
    invoke<PlaylistImportResult>('import_playlist_json', {
      path,
      name: name ?? null,
    }),

  // ---- listen statistics ----------------------------------------------
  recordListenStart: (trackId: number) =>
    invoke<void>('record_listen_start', { trackId }),
  recordListenEnd: (input: {
    trackId: number
    listenedMs: number
    durationMs?: number | null
    endReason: ListenEndReason
  }) =>
    invoke<ListenEndResult | null>('record_listen_end', {
      trackId: input.trackId,
      listenedMs: input.listenedMs,
      durationMs: input.durationMs ?? null,
      endReason: input.endReason,
    }),
  getListenStatisticsEnabled: () =>
    invoke<boolean>('get_listen_statistics_enabled'),
  setListenStatisticsEnabled: (enabled: boolean) =>
    invoke<void>('set_listen_statistics_enabled', { enabled }),
  getTrackListenStats: (trackId: number) =>
    invoke<TrackListenStats | null>('get_track_listen_stats', { trackId }),
  listListenStats: () => invoke<TrackListenStats[]>('list_listen_stats'),
  rebuildListenStats: () => invoke<void>('rebuild_listen_stats'),
  clearListenStatistics: () => invoke<void>('clear_listen_statistics'),

  // ---- Last.fm ---------------------------------------------------------
  getLastFmStatus: () => invoke<LastFmStatus>('get_lastfm_status'),
  startLastFmAuth: () => invoke<LastFmStatus>('start_lastfm_auth'),
  completeLastFmAuth: () => invoke<LastFmStatus>('complete_lastfm_auth'),
  cancelLastFmAuth: () => invoke<LastFmStatus>('cancel_lastfm_auth'),
  setLastFmEnabled: (enabled: boolean) =>
    invoke<LastFmStatus>('set_lastfm_enabled', { enabled }),
  disconnectLastFm: (pendingAction?: LastFmPendingAction) =>
    invoke<LastFmStatus>('disconnect_lastfm', {
      pendingAction: pendingAction ?? null,
    }),
  openLastFmProfile: () => invoke<void>('open_lastfm_profile'),
  flushLastFmQueue: () => invoke<void>('flush_lastfm_queue'),
  lastFmAttemptStarted: (attemptId: string, trackId: number) =>
    invoke<void>('lastfm_attempt_started', { attemptId, trackId }),
  lastFmAttemptQualified: (attemptId: string, listenedMs: number) =>
    invoke<void>('lastfm_attempt_qualified', { attemptId, listenedMs }),
  lastFmAttemptEnded: (attemptId: string) =>
    invoke<void>('lastfm_attempt_ended', { attemptId }),
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

export interface SyncError {
  message: string
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

export function onSyncError(cb: (error: SyncError) => void): Promise<UnlistenFn> {
  return listen<SyncError>('sync:error', event => cb(event.payload))
}

export function onDownloadProgress(
  cb: (p: DownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<DownloadProgress>('download:progress', e => cb(e.payload))
}

export function onLastFmStatusChanged(
  cb: (status: LastFmStatus) => void,
): Promise<UnlistenFn> {
  return listen<LastFmStatus>('lastfm:status_changed', event => cb(event.payload))
}

export function onPlaylistDownloadProgress(
  cb: (p: PlaylistDownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<PlaylistDownloadProgress>(
    'download_playlist:progress',
    e => cb(e.payload),
  )
}

export function onCacheTracksProgress(
  cb: (p: CacheTracksProgress) => void,
): Promise<UnlistenFn> {
  return listen<CacheTracksProgress>(
    'cache_tracks:progress',
    e => cb(e.payload),
  )
}

export function onCacheChanged(
  cb: (p: CacheChanged) => void,
): Promise<UnlistenFn> {
  return listen<CacheChanged>('cache:changed', e => cb(e.payload))
}

export function onAuthRevoked(cb: () => void): Promise<UnlistenFn> {
  return listen('auth:revoked', () => cb())
}
