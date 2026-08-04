# Frontend (`src/`) — agent notes

React 19 + Zustand + Vite + Tailwind. Import alias: `@/` → `src/`.

## Bootstrap

[App.tsx](App.tsx): local `auth_status` (`session.enc` + SQLite profile) → login or hydrate session → `listTracks` / `listPlaylists` → UI ready → background reconnect loop (`useTelegramReconnect`: `refresh_auth` + backoff + browser `online`/`offline`) then `sync_saved_music`. Network failures leave the cached library intact; `auth:revoked` forces login.

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
| `connectivity-store` | Telegram reachability (`connecting` / `online` / `offline`) |
| `library-store` | Track list |
| `playlists-store` | Liked + custom playlists, selection |
| `listen-stats-store` | Per-track listen aggregates (smart playlists) |
| `player-store` | Queue, current track, playback flags |
| `cache-store` | Which tracks are fully present in app audio cache |
| `playlist-jobs-store` | In-flight playlist download/cache jobs + result queue |
| `shuffle-store` / `repeat-store` | Playback modes |
| `fullscreen-store` | Fullscreen player UI |

Prefer updating existing stores over adding parallel state.

Proxy / connection settings are edited via Settings and the login-screen panel; they persist in the backend (`app_settings`) and are not kept in Zustand.

## UI conventions

- Components by area: `components/audio/`, `playlist/`, `auth/`, `ui/`.
- Reuse `components/ui/` primitives; match existing Tailwind patterns.
- Keep components thin: data via stores + `api`, not ad-hoc backend calls.
