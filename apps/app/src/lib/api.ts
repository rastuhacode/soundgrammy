import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AuthOutcome,
  AuthStatus,
  AuthUser,
  CustomPlaylistSummary,
  PlaylistsBundle,
  QrOutcome,
  SyncResult,
  Track,
  TrackMetadata,
} from "@/types";

export interface Profile {
  tgUserId: number;
  firstName: string;
  lastName: string | null;
  username: string | null;
}

// ---- auth ----------------------------------------------------------------

export const api = {
  authStatus: () => invoke<AuthStatus>("auth_status"),
  phoneSendCode: (phone: string) => invoke<void>("phone_send_code", { phone }),
  phoneSignIn: (code: string) => invoke<AuthOutcome>("phone_sign_in", { code }),
  phoneCheckPassword: (password: string) =>
    invoke<AuthUser>("phone_check_password", { password }),
  qrStart: () => invoke<QrOutcome>("qr_start"),
  qrPoll: () => invoke<QrOutcome>("qr_poll"),
  qrCheckPassword: (password: string) =>
    invoke<AuthUser>("qr_check_password", { password }),
  logout: () => invoke<void>("logout"),

  // ---- library ----------------------------------------------------------
  syncSavedMusic: () => invoke<SyncResult>("sync_saved_music"),
  listTracks: () => invoke<Track[]>("list_tracks"),
  getProfile: () => invoke<Profile | null>("get_profile"),
  syncStatus: () => invoke<string | null>("sync_status"),

  // ---- media ------------------------------------------------------------
  getTrackSource: (trackId: number) =>
    invoke<string>("get_track_source", { trackId }),
  prefetchTrack: (trackId: number) =>
    invoke<void>("prefetch_track", { trackId }),
  getTrackThumbnail: (trackId: number) =>
    invoke<string | null>("get_track_thumbnail", { trackId }),
  getUserAvatar: () => invoke<string | null>("get_user_avatar"),
  trackMetadata: (trackId: number) =>
    invoke<TrackMetadata>("track_metadata", { trackId }),

  // ---- playlists --------------------------------------------------------
  listPlaylists: () => invoke<PlaylistsBundle>("list_playlists"),
  createPlaylist: (input: {
    name: string;
    thumbnailData?: string | null;
    thumbnailMime?: string | null;
  }) =>
    invoke<CustomPlaylistSummary>("create_playlist", {
      name: input.name,
      thumbnailData: input.thumbnailData ?? null,
      thumbnailMime: input.thumbnailMime ?? null,
    }),
  updatePlaylist: (input: {
    playlistId: number;
    name?: string | null;
    thumbnailData?: string | null;
    thumbnailMime?: string | null;
    clearThumbnail?: boolean;
  }) =>
    invoke<CustomPlaylistSummary>("update_playlist", {
      playlistId: input.playlistId,
      name: input.name ?? null,
      thumbnailData: input.thumbnailData ?? null,
      thumbnailMime: input.thumbnailMime ?? null,
      clearThumbnail: input.clearThumbnail ?? null,
    }),
  deletePlaylist: (playlistId: number) =>
    invoke<void>("delete_playlist", { playlistId }),
  getPlaylistThumbnail: (playlistId: number) =>
    invoke<string | null>("get_playlist_thumbnail", { playlistId }),
  addTrackToPlaylist: (playlistId: number, trackId: number) =>
    invoke<void>("add_track_to_playlist", { playlistId, trackId }),
  removeTrackFromPlaylist: (playlistId: number, trackId: number) =>
    invoke<void>("remove_track_from_playlist", { playlistId, trackId }),
  toggleLike: (trackId: number) => invoke<number[]>("toggle_like", { trackId }),
};

/** Turns an absolute cache path into an `asset:` URL for `<audio>`/`<img>`. */
export function fileSrc(path: string): string {
  return convertFileSrc(path);
}

// ---- events --------------------------------------------------------------

export interface SyncProgress {
  done: number;
  total: number;
}

export interface DownloadProgress {
  trackId: number;
  received: number;
  total: number;
}

export function onSyncStart(cb: () => void): Promise<UnlistenFn> {
  return listen("sync:start", () => cb());
}

export function onSyncProgress(
  cb: (p: SyncProgress) => void,
): Promise<UnlistenFn> {
  return listen<SyncProgress>("sync:progress", (e) => cb(e.payload));
}

export function onSyncDone(cb: () => void): Promise<UnlistenFn> {
  return listen("sync:done", () => cb());
}

export function onDownloadProgress(
  cb: (p: DownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<DownloadProgress>("download:progress", (e) => cb(e.payload));
}
