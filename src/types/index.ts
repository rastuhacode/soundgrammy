// Shared types mirroring the Rust command payloads (serde field names).

import { z } from 'zod'

export const trackSchema = z.object({
  id: z.number().int(), tg_user_id: z.number().int(), file_id: z.string(), file_unique_id: z.string(),
  title: z.string().nullable(), performer: z.string().nullable(), duration: z.number().nullable(),
  source: z.string(), mime_type: z.string().nullable(), file_size: z.number().nullable(), created_at: z.string(),
})
export type Track = z.infer<typeof trackSchema>

export const likedPlaylistSchema = z.object({ id: z.number().int(), trackIds: z.array(z.number().int()), updatedAt: z.string() })
export type LikedPlaylist = z.infer<typeof likedPlaylistSchema>
export const customPlaylistSchema = likedPlaylistSchema.extend({ name: z.string() })
export type CustomPlaylistSummary = z.infer<typeof customPlaylistSchema>
export const playlistsBundleSchema = z.object({ liked: likedPlaylistSchema, custom: z.array(customPlaylistSchema) })
export type PlaylistsBundle = z.infer<typeof playlistsBundleSchema>

export interface AuthUser {
  id: number
  firstName: string
  lastName: string | null
  username: string | null
  phone: string | null
}

/** Session shape consumed by the UI (kept close to the old web payload). */
export interface SessionPayload {
  tgUserId: number
  firstName: string
  lastName: string | null
  username: string | null
  phone?: string | null
}

export function authUserToSession(user: AuthUser): SessionPayload {
  return {
    tgUserId: user.id,
    firstName: user.firstName,
    lastName: user.lastName,
    username: user.username,
    phone: user.phone,
  }
}

export interface AuthStatus {
  authorized: boolean
  user: AuthUser | null
}

export interface SyncResult {
  changed: boolean
  total: number
  lastSyncAt: string | null
}

export type AuthOutcome
  = | { status: 'authorized', user: AuthUser }
    | { status: 'passwordRequired', hint: string | null }

export type PhoneSendCodeOutcome
  = | { status: 'codeSent' }
    | { status: 'authorized', user: AuthUser }

export type QrOutcome
  = | { status: 'waiting', url: string, expires: number }
    | { status: 'passwordRequired', hint: string | null }
    | { status: 'authorized', user: AuthUser }

export interface SerializedAttribute {
  type: string
  [key: string]: unknown
}

export interface TrackMetadata {
  track: {
    title: string | null
    performer: string | null
    duration: number | null
    mimeType: string | null
    fileSize: number | null
    source: string
    fileId: string
    fileUniqueId: string
    createdAt: string
  }
  document: {
    id: string
    dcId: number
    mimeType: string | null
    size: number | null
    hasRemoteThumb: boolean
    attributes: SerializedAttribute[]
  }
}

export type ListenEndReason
  = | 'completed'
    | 'skipped'
    | 'replaced'
    | 'stopped'
    | 'interrupted'

export const trackListenStatsSchema = z.object({
  track_id: z.number().int(), starts: z.number(), qualified_plays: z.number(), completes: z.number(),
  early_skips: z.number(), total_listened_ms: z.number(),
  first_played_at_ms: z.number().nullable(), last_played_at_ms: z.number().nullable(), likeness: z.number(),
})
export type TrackListenStats = z.infer<typeof trackListenStatsSchema>

export interface ListenEndResult {
  qualified: boolean
  early_skip: boolean
  listened_eff_ms: number
  stats: TrackListenStats
}

export type LastFmAuthState
  = | 'unavailable_in_build'
    | 'disconnected'
    | 'requesting_token'
    | 'waiting_for_browser_approval'
    | 'exchanging_session'
    | 'connected'
    | 'needs_reauthentication'
    | 'error'

export interface LastFmSafeIssue {
  kind: string
  code: number | null
  message: string
  atMs: number
}

export interface LastFmQueueSummary {
  username: string
  count: number
}

export interface LastFmStatus {
  state: LastFmAuthState
  username: string | null
  enabled: boolean
  pendingCount: number
  retainedQueues: LastFmQueueSummary[]
  lastScrobbleAtMs: number | null
  lastError: LastFmSafeIssue | null
  lastMetadataWarning: LastFmSafeIssue | null
}

export type LastFmPendingAction = 'retain' | 'delete'

export const cacheSettingsSchema = z.object({ limitBytes: z.number().nonnegative(), ttlSecs: z.number().nonnegative() })
export type CacheSettings = z.infer<typeof cacheSettingsSchema>
export const cacheUsageSchema = z.object({ usedBytes: z.number().nonnegative(), limitBytes: z.number().nonnegative(), fileCount: z.number().int().nonnegative() })
export type CacheUsage = z.infer<typeof cacheUsageSchema>

export type BounceProfileResponse
  = | {
    status: 'ready'
    algorithmVersion: number
    frameMs: number
    durationMs: number
    loudnessData: string
    onsetData: string
  }
  | { status: 'unavailable' }

export interface ProxySettings {
  enabled: boolean
  server: string
  port: number
  secret: string
}

export interface ProxySettingsView extends ProxySettings {
  active: boolean
  applyError: string | null
  link: string | null
  telegramOnline: boolean
}

export interface CacheChanged {
  trackIds: number[]
  cached: boolean
  cleared: boolean
}

export interface PlaylistDownloadSucceeded {
  trackId: number
  title: string | null
  performer: string | null
  fileName: string
}

export interface PlaylistDownloadFailed {
  trackId: number
  title: string | null
  performer: string | null
  error: string
}

export interface PlaylistDownloadResult {
  folderPath: string | null
  succeeded: PlaylistDownloadSucceeded[]
  failed: PlaylistDownloadFailed[]
}

export interface PlaylistDownloadProgress {
  jobId: string
  current: number
  total: number
  trackId: number
}

export interface CacheTracksProgress {
  jobId: string
  current: number
  total: number
  trackId: number
}

export type PlaylistRecipeSource
  = | { kind: 'liked' }
    | { kind: 'custom', playlistId: number }

export interface PlaylistImportSucceeded {
  fileUniqueId: string
  title: string | null
  performer: string | null
}

export interface PlaylistImportFailed {
  fileUniqueId: string
  title: string | null
  performer: string | null
  reason: string
}

export interface PlaylistImportPreview {
  suggestedName: string
  succeeded: PlaylistImportSucceeded[]
  failed: PlaylistImportFailed[]
}

export interface PlaylistImportResult {
  playlistId: number
  playlistName: string
  succeeded: PlaylistImportSucceeded[]
  failed: PlaylistImportFailed[]
}
