# Frontend (`src/`) — agent notes

React 19 + Zustand + Vite + Tailwind. Import alias: `@/` → `src/`.

## Bootstrap

[App.tsx](App.tsx): `auth_status` → login or hydrate session → `listTracks` / `listPlaylists` → background `sync_saved_music`. Sync failures leave the cached library intact.

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
| `library-store` | Track list |
| `playlists-store` | Liked + custom playlists, selection |
| `listen-stats-store` | Per-track listen aggregates (smart playlists) |
| `player-store` | Queue, current track, playback flags |
| `cache-store` | Which tracks are fully present in app audio cache |
| `playlist-jobs-store` | In-flight playlist download/cache jobs + result queue |
| `shuffle-store` / `repeat-store` | Playback modes |
| `fullscreen-store` | Fullscreen player UI |

Prefer updating existing stores over adding parallel state.

## UI conventions

- Components by area: `components/audio/`, `playlist/`, `auth/`, `ui/`.
- Reuse `components/ui/` primitives; match existing Tailwind patterns.
- Keep components thin: data via stores + `api`, not ad-hoc backend calls.
