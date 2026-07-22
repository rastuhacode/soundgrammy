// Shared types mirroring the Rust command payloads (serde field names).

export interface Track {
  id: number
  tg_user_id: number
  file_id: string
  file_unique_id: string
  title: string | null
  performer: string | null
  duration: number | null
  source: string
  mime_type: string | null
  file_size: number | null
  created_at: string
}

export interface LikedPlaylist {
  id: number
  trackIds: number[]
  updatedAt: string
}

export interface CustomPlaylistSummary {
  id: number
  name: string
  trackIds: number[]
  hasThumbnail: boolean
  updatedAt: string
}

export interface PlaylistsBundle {
  liked: LikedPlaylist
  custom: CustomPlaylistSummary[]
}

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

export interface TrackListenStats {
  track_id: number
  starts: number
  qualified_plays: number
  completes: number
  early_skips: number
  total_listened_ms: number
  first_played_at_ms: number | null
  last_played_at_ms: number | null
  likeness: number
}

export interface ListenEndResult {
  qualified: boolean
  early_skip: boolean
  listened_eff_ms: number
  stats: TrackListenStats
}

export interface CacheSettings {
  limitBytes: number
  ttlSecs: number
}

export interface CacheUsage {
  usedBytes: number
  limitBytes: number
  fileCount: number
}

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
