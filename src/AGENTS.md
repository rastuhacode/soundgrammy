# Frontend (`src/`) — agent notes

React 19 + Zustand + Vite + Tailwind. Import alias: `@/` → `src/`.

## Bootstrap

[use-app-session.ts](hooks/use-app-session.ts) owns auth, hydration, and subscriptions; [App.tsx](App.tsx) owns layout. Local `auth_status` (`session.enc` + SQLite profile) → login or hydrate session → `listTracks` / `listPlaylists` → UI ready → background reconnect loop (`useTelegramReconnect`: `refresh_auth` + backoff + browser `online`/`offline`) then `sync_saved_music`. Network failures leave the cached library intact; `auth:revoked` forces login.

## Boundaries

- All backend access through [lib/api.ts](lib/api.ts) (`invoke` + event listeners).
- Shared payloads in [types/index.ts](types/index.ts) (mirror Rust serde shapes).
- Do not call Tauri `invoke` from components/stores directly; extend `api` instead.
- UI holds display session fields only (`AuthUser` / `SessionPayload`) — never MTProto material.

## State

Zustand stores under `stores/`:

| Store | Owns |
|-------|------|
| `session-store` | Logged-in user display fields |
| `connectivity-store` | Telegram reachability and one shared automatic/manual sync coordinator |
| `library-store` | Track list |
| `playlists-store` | Liked + custom playlists, selection, serialized mutations and refreshes |
| `listen-stats-store` | Per-track listen aggregates (smart playlists) |
| `player-store` | Native queue/current-track/intent mirror; actions send native commands |
| `cache-store` | Which tracks are fully present in app audio cache |
| `playlist-jobs-store` | In-flight playlist download/cache jobs + result queue |
| `shuffle-store` / `repeat-store` | Read-only native playback mode mirrors |
| `fullscreen-store` | Fullscreen player UI |

Prefer updating existing stores over adding parallel state.

Proxy / connection settings are edited via Settings and the login-screen panel; they persist in the backend (`app_settings`) and are not kept in Zustand.

## UI conventions

- Components by area: `components/audio/`, `playlist/`, `auth/`, `ui/`.
- Reuse `components/ui/` primitives; match existing Tailwind patterns.
- Keep components thin: data via stores + `api`, not ad-hoc backend calls.

## Async ownership and domain rules

- Capture the session generation before account-scoped work; discard results after logout/account change.
- Playlist mutations belong to `playlists-store`; components must not write whole bundles after awaiting commands.
- `lib/library-hydration.ts` loads each domain independently. Statistics/cache failures must not block tracks or playlists.
- Positional row selection belongs to an exact ordered membership snapshot and resets when that snapshot changes.
- `lib/playlists.ts` owns built-in playlist metadata/types/resolvers; `lib/playlist-track-actions.ts` owns pure action rules.
- Settings mutations belong to their domain stores and use account-scoped queues.
- Library hydration and `lib/playlist-playback.ts` coordinate domain commits with playback mirrors; library/playlist stores do not import the player store.
- Playlist UI passes `{ track, sourceIndex }` entries instead of parallel track/position arrays.
- Playback composition provides both an audio engine and a player command port; native ports share one account-scoped IPC queue.
- Shared playback/transport contracts live in `types/`; they must not import stores or React hooks.
- `test-support/legacy-*` contains historical web playback fixtures only. Production queue policy and listen accounting belong to Rust; frontend fixture tests do not validate native policy.
