# SoundGrammy — agent notes

Desktop music player (Tauri 2) over Telegram MTProto (grammers). Local-first: library and playlists live in SQLite; Telegram saved music is the remote source of truth.

Human-facing setup: see [README.md](README.md).

## Layout

| Path | Role |
|------|------|
| `src/` | React UI (Vite, Zustand, Tailwind) |
| `src-tauri/` | Rust backend (commands, SQLite, grammers, session, streaming) |
| `docs/` | Architecture and security detail (read on demand) |

## Commands

```bash
bun i
bun tauri:dev
bun lint
bun typecheck
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

- [docs/architecture.md](docs/architecture.md) — data flow, sync, media
- [docs/security.md](docs/security.md) — credentials, session, high-risk areas
