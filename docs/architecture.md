# Architecture

SoundGrammy is a local-first Tauri music player with a React interface and a process-owned Rust playback service. Desktop releases are supported; Android and iOS playback integration is implemented but still requires mobile build/device validation as described in [mobile background playback](mobile-background-playback.md). The UI never talks to Telegram directly; Rust owns MTProto, SQLite, decoding, queue policy, and listening accounting.

```mermaid
flowchart LR
  UI[React and Zustand mirrors] -->|api.ts / Tauri commands| Cmds[Rust commands]
  Cmds --> Player[Native playback service]
  Player -->|revisioned snapshots / events| UI
  OS[OS media controls] <-->|native platform adapters| Player
  Player --> Queue[Queue / repeat / shuffle]
  Player --> Audio[Verified source / decoder / CPAL output]
  Player --> Activity[Listen accounting / Last.fm qualification]
  Cmds --> Db[SQLite]
  Activity --> Db
  Audio --> Cache[Audio cache / TrackStream]
  Cache --> Tg[ferogram MTProto]
  Cmds --> Tg
  Tg --> Session[Encrypted session / OS keyring]
```


## Bootstrap

1. `auth_status` — **local only** (`session.enc` + SQLite profile). No MTProto. Unauthorized → login UI.
2. On authorized: `useAppSession` hydrates the session and independently loads tracks, playlists, listen statistics, and cache status → UI **ready**. Display metadata loads from SQLite in pages of at most 500 rows, all at one library revision. Optional statistics/cache failures do not block library hydration; track/playlist failures expose a retry.
3. Automatic reconnect and tower sync share the `connectivity-store` coordinator and use soft sync. First indexing and confirmed full reindexing replace the library with a progress loader until metadata indexing and final local hydration complete. Soft-sync additions publish `library:batch` events while the existing library stays visible. A response-driven reload repairs missed events on sync `changed`, or after a failed scan that may have committed partial progress.
4. Network timeouts / unreachable preserve cached tracks and the local session; sync-dot shows offline / connecting and keeps retrying.
5. Logout or account change invalidates account-scoped responses with a session generation and clears playback mirrors, job presentation, statistics, cache mirrors, and thumbnail memory. Server-proven session death (`AUTH_KEY_*` / `SESSION_REVOKED`, etc.) clears local session, emits `auth:revoked`, UI returns to login.
6. Sync errors retain the previous cached tracks and successfully committed new pages. Reconnect retries from a durable checkpoint after validating an overlapping page. Removals wait for a verified complete scan.

Optional **MTProto proxy** (tg-ws-proxy compatible: server / port / secret or `tg://proxy?…`) is stored in SQLite `app_settings` and applied when building the ferogram client. Changing proxy settings rebuilds the client in-process. If a configured proxy fails at startup, the app falls back to a direct connection so the login UI can still load and the user can disable the proxy.

The UI reconnect loop refreshes library/auth presentation. Playback-critical downloads have native retry and connection recovery; background playback does not depend on browser connectivity events. See [streaming](streaming.md).

## Data ownership

Saved music sync first checks `account.getSavedMusicIds` using Telegram's ordered
64-bit list hash. The manifest hash and metadata completeness are separate durable
state: an unchanged manifest never skips an interrupted scan. A complete unchanged
catalogue skips metadata requests when unchanged. Soft sync compares that manifest
with local tracks, requests pages likely to contain missing IDs, and stores only
new metadata. Manifest ordinals are hints: unmatched IDs trigger a search of the
remaining pages. Cached relative order is retained, and additions are inserted at
their observed page positions. Soft sync does not refresh existing metadata or
schedule full scans based on elapsed time. Settings → Music library → Fully reindex
tracks explicitly refetches all metadata and Telegram order after confirmation.
Initial/full indexing scans `users.getSavedMusic` in batches of 100. Each page and
its cursor commit together. A final manifest check and exact SQL membership
validation precede pruning, without a bind parameter per track.
Flood waits exceeding ferogram's automatic sleep threshold persist a per-account
retry deadline; subsequent syncs and process restarts respect that cooldown.
Full-reindex intent is persisted by `prepare_full_reindex` before network/auth
refresh, so offline failures and restarts retain the loading state. Track IDs are
reused to preserve playlist memberships, likes, statistics and cache ownership for
tracks still saved on Telegram. Existing completed catalogues remain visible;
legacy partial initial indexes are hidden. The legacy XOR hash is not reused.

The frontend retains the complete **local display metadata** index. Search keeps
its existing locale-aware substring semantics; playlist resolution, sorting,
selection, whole-playlist jobs, and native queue construction include every locally
indexed track, independent of the virtual viewport. Local IPC reads are paged and
omit stored MTProto document JSON. This avoids coupling feature completeness to
scrolling without replacing these operations with different SQL search semantics.
Initial/full indexing never exposes a partial catalogue through display pages or
batch events. The app's progress loader remains until the final local snapshot
loads, and keeps Settings/connection controls available. Soft-sync status and
progress appear in the tower menu, with no loader or track-view indexing banner.
Batch revisions discard stale events, and snapshot repairs retry rather than
letting late completion events cancel them. Failed full scans remain hidden and
resume after a retry/reconnect; soft scans resume by recomputing missing IDs.

| Data | Source of truth | Local store |
|------|-----------------|-------------|
| Saved / profile music | Telegram | SQLite tracks (synced) |
| Custom playlists | App | SQLite |
| Liked playlist | App | SQLite |
| Playback queue / transport | Rust audio session | Native in-memory queue; Zustand mirrors snapshots (not restored across restart) |
| Repeat/shuffle preferences | Rust audio session | SQLite `playback_preferences` |
| App gain / mute controls | Frontend preference, applied to native output | localStorage; reapplied on UI attachment |
| Listen statistics | App (listen behaviour) | SQLite events + aggregates ([listen-statistics.md](./listen-statistics.md)) |
| Last.fm account | Last.fm | Session key in OS keyring; username/settings in SQLite |
| Pending Last.fm scrobbles | App (qualified playback attempts) | SQLite immutable queue |

**Playlist JSON recipe** (`export_playlist_json` / `analyze_playlist_json` / `import_playlist_json`): same-account cross-device sync for Liked and custom playlists. File contains ordered Telegram document ids (`file_unique_id`) and exporter `tgUserId`. Import is a prepare-then-create flow in the Create playlist dialog (analyze matches first; name can be edited). Import always creates a new custom playlist (duplicate names allowed); other-account files are rejected. Distinct from **Download playlist** (audio files + M3U under Downloads).

## Frontend playlist ownership

`playlists-store` serializes playlist mutations and refreshes within an account lifetime. Components express mutation intent; they do not merge captured bundles after asynchronous commands. Creating a playlist with queue tracks and removing a viewed set of custom-playlist positions are SQLite transactions. Bulk removal validates the expected ordered membership before deleting; bulk like actions apply a desired state and report failed track IDs for retry.

Selection uses the exact ordered membership snapshot and clears if that snapshot changes, preventing positions from silently targeting other tracks. Stable persistent membership IDs are not yet exposed by the IPC contract. Pure playlist metadata and action rules live under `src/lib`; desktop context and touch dropdown menus share one action tree. The table, selection, and playback preparation share explicit `{ track, sourceIndex }` entries. Focused hooks own derived tracks, selection, and action orchestration; the toolbar receives separate playback or selection controls. Backend library/playlist commits reconcile the native queue before acknowledging completion; frontend snapshots mirror the result. Reorder commands include the viewed order and optional exact move, and preserve persistent membership rows. Playlist rename updates its native source label; deletion retains playback as a manual queue. Historical web playback algorithms are isolated under `src/test-support` and are not used by the production player.

## Media

- **App cache**: audio under the app cache dir (`audio/{file_unique_id}.{ext}`). Used for in-app playback. Subject to Settings size limit / TTL / clear. Thumbnail border (greyish → primary) reflects cache status.
- **Download (export)**: copies a track into the system Downloads folder (`SoundGrammy/…`). Not removed by clear cache or eviction. Bulk export uses a dated subfolder.
- **Download playlist** (`download_playlist`): writes `Downloads/SoundGrammy/<playlist name>/` with audio files plus a UTF-8 `.m3u8` (relative paths). Allowed for All tracks, Liked, and custom playlists (not Popular/Recent). Sequential per-track `ensure_audio` → copy (same as single-track download); does **not** use the bulk Cache size pre-check. Partial success: failures are skipped and reported; M3U lists only files that landed. If the M3U write fails after audio copies succeed, the command returns the folder, succeeded/failed entries and an artifact warning. Bulk exports use the same partial-result contract. Completed desktop copies publish atomically without replacing concurrent exports. Job progress is keyed by `jobId` so parallel playlist downloads and playlist switches keep correct UI state (`playlist-jobs-store`). Cache and download jobs share busy ownership and cleanup; app-level job presentation owns result dialogs and opening exported folders, including while compact navigation shows the sidebar.
- **Cached playback path**: Rust opens the local audio file directly. Images continue using `fileSrc()` (`asset:` URLs).
- **Uncached (streamed)**: the Rust audio source reads verified byte ranges from `TrackStream`. Symphonia decodes audio and CPAL sends PCM to the output device. No audio bytes cross frontend IPC.
- The native engine publishes presentation time and timestamp-mapped downloaded ranges to the UI. Missing bytes remain unavailable until the downloader verifies them.
- Completing a stream marks the track cached. Explicit **Cache** also fills app cache without writing to Downloads.
- See [native audio](./native-audio.md) and [streaming](./streaming.md) for playback and download lifetimes.

## Events

| Event | Meaning |
|-------|---------|
| `sync:start` / `sync:progress` / `sync:done` | Saved-music sync lifecycle |
| `library:batch` | Revisioned committed display metadata, explicit removals, and indexing completeness |
| `auth:revoked` | Local session cleared after server-proven auth death |
| `download:progress` | Per-track download bytes / ranges |
| `download_playlist:progress` | Playlist download slot progress (`jobId` / `current` / `total` / `trackId`) |
| `cache_tracks:progress` | Bulk cache job progress when a `jobId` is provided |
| `cache:changed` | Track(s) entered/left app cache, or full clear |
| `audio:state` | Native transport snapshot; session state included on queue/policy changes |
| `audio:event` | Natural completion notification; progression already belongs to Rust |
| `listen:stats` | Native listen aggregate update |
| `lastfm:status_changed` | Safe account/auth/queue status changed |

Listeners live in `src/lib/api.ts`. Shared playback schemas live in `src/types`; core library, playlist, statistics, and cache-setting command responses are validated at IPC ingress. Contract diagnostics record field paths and error codes without recording raw payloads.

## Where to change what

| Concern | Start here |
|---------|------------|
| New IPC | `commands.rs` → `lib.rs` → `src/lib/api.ts` |
| Track / playlist persistence | `db.rs` |
| Telegram sync | `telegram/saved_music.rs` |
| Auth flows | `telegram/auth.rs` + login UI |
| Native queue / playback policy | `src-tauri/src/audio/session.rs`, `audio/shuffle.rs`, `audio/mod.rs` |
| Player UI / queue mirror | `src/stores/player-store.ts`, `src/components/audio/`, `src/hooks/audio/native-engine.ts` |
| OS controls / mobile lifecycle | `src-tauri/src/audio/media/`, `audio/lifecycle.rs`, `audio/ios.rs`, Android `PlaybackService.kt` |
| Listen statistics | `listen_stats.rs`, `db.rs`, `audio/activity.rs`, `stores/listen-stats-store.ts` |
| Playlist tracklist (table, sort, selection, context menu) | `components/playlist/` (`PlaylistView`, `PlaylistTracksTable`, `track-actions`) |

## Playback queue

- Session playback order lives in Rust `audio/session.rs` (`tracks`, cursor, source, and base order). `player-store` mirrors acknowledged native snapshots; queue actions send commands to Rust.
- Visible/editable via the queue control on the player (`components/audio/queue/`).
- Edits (reorder / add / remove / clear up next) clear the source label so the queue is no longer presented as the original playlist.
- Survives UI suspension/recreation while the native process remains alive. Queue/cursor/position are not restored after process termination; a fresh process starts idle. **Save as playlist** preserves track order, not a playback checkpoint (full / from here / up next scopes).
- Play next / Add to end from track context and bulk actions insert into the session queue; whole-playlist Play still replaces it.

## Playlist boundaries

- **All tracks** (`id: all`) — virtual view of the synced library; not a DB playlist. Immutable membership (no remove-from-playlist). Track order follows Telegram sync (`track_position`); not drag-reorderable.
- **Liked** (`id: liked`) — app-owned; membership via `toggle_like` only (not `add_track_to_playlist` / remove-from-playlist UI). Unique membership. Custom order persisted with `reorder_playlist_tracks`.
- **Popular** (`id: popular`) / **Recent** (`id: recent`) — virtual smart playlists from listen stats ∩ library. Ordered by likeness / last played. Immutable membership; not drag-reorderable.
- **Custom playlists** — editable membership; the same track may appear more than once as distinct ordered entries. Context menu and bulk actions may show “Remove from playlist” (removes one occurrence by position). Track order persisted with `reorder_playlist_tracks`.

Tracklist actions are gated in `src/lib/playlist-track-actions.ts` so non-custom playlists never expose remove-from-playlist. Drag-reorder is enabled only for Liked/custom when search and column sort are clear and selection mode is off.

See [Audio engine boundary](audio-engine.md) for transport ownership, implementation selection, and the native proxy contract.

## OS media controls

The [native media bridge](./native-media-bridge.md) publishes Rust playback state to macOS/iOS MediaPlayer, Windows SMTC, Linux MPRIS, and Android MediaSession. Native callbacks share the Rust command queue with UI actions and continue across WebView reloads.

## Playback lifetime and persistence

Backgrounding, UI recreation, and process termination are different boundaries:

| Boundary | Current behavior |
| --- | --- |
| React unmount or WebView reload | Detach subscriptions and invalidate old UI commands; native playback continues |
| UI returns in the same process | Fetch revisioned transport/session snapshots and listen aggregates; do not start another attempt |
| Mobile background / screen lock | Android foreground service or iOS audio session supports native playback; device acceptance remains outstanding |
| Explicit exit, OS process death, or force-stop | In-memory queue, position, and active attempt are lost; next launch starts idle |
| Logout / revoked Telegram session | Explicitly clear the playback session and OS metadata |

SQLite retains the library, playlists, repeat/shuffle preferences, recorded listen history,
and already-queued Last.fm scrobbles. An unfinished listen attempt is not a durable
checkpoint and may be lost on abrupt termination. Persistent Telegram authentication
is separate from the in-memory playback session.

Queue/position restoration is not implemented. Adding it would require durable queue
membership/order and cursor, position checkpoints, validation against the current
library/account, and an explicit policy for incomplete listen attempts. Restoring the
UI in a paused state would allow an explicit Play action without automatically starting
audio on launch. Such restoration cannot keep audio running while the process is dead.
