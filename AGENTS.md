# SoundGrammy — agent notes

Local-first Tauri 2 music player for desktop and Android. SQLite stores the library and playlists. Telegram profile music is the remote source of truth, accessed through ferogram.

Human-facing setup: see [README.md](README.md).

## Layout

| Path | Role |
|------|------|
| `src/` | React UI (Vite, Zustand, Tailwind) |
| `src-tauri/` | Rust backend (commands, SQLite, ferogram, session, streaming) |
| `docs/` | Architecture and security detail (read on demand) |

## Commands

```bash
bun i
bun tauri:dev
bun lint
bun typecheck
bun run test
cd src-tauri && cargo check
```

## Hard rules

- Frontend ↔ backend only via Tauri commands and events (`src/lib/api.ts` ↔ `commands.rs`).
- Do not put MTProto, session crypto, or Telegram credentials in the frontend.
- Keep diffs minimal on auth, session, and crypto (`session.rs`, `config.rs`, `telegram/auth.rs`).
- Prefer extending existing modules/stores over new abstractions.
- Package manager is Bun (not npm/yarn).

## Nested notes

- [src/AGENTS.md](src/AGENTS.md) — frontend conventions
- [src-tauri/AGENTS.md](src-tauri/AGENTS.md) — Rust / Telegram conventions

## Docs

- [docs/architecture.md](docs/architecture.md) — data ownership, sync, and code entry points
- [docs/native-audio.md](docs/native-audio.md) — native queue, UI reattachment, and decoder contracts
- [docs/streaming.md](docs/streaming.md) — native playback over verified download ranges
- [docs/listen-statistics.md](docs/listen-statistics.md) — listen events, aggregates, likeness
- [docs/security.md](docs/security.md) — credentials, session, high-risk areas
