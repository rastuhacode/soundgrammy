# Backend (`src-tauri/`) — agent notes

Rust Tauri 2 app: Telegram via grammers, library in SQLite, media cache + stream protocol.

## Module map

| Module | Role |
|--------|------|
| `src/commands.rs` | Tauri IPC handlers |
| `src/lib.rs` | App setup, command registration, `stream` protocol |
| `src/db.rs` | SQLite (`library.db`) |
| `src/telegram/` | Client, auth, saved music sync, download |
| `src/session.rs` | Encrypted session at rest + keyring |
| `src/config.rs` | `TELEGRAM_API_ID` / `TELEGRAM_API_HASH` |
| `src/streaming.rs` | Range streaming for uncached tracks |
| `src/cache.rs` | On-disk media/thumbnail paths |
| `src/state.rs` | Shared `AppState` |

## Adding a command

1. Implement in `commands.rs`.
2. Register in `lib.rs` `invoke_handler!`.
3. Mirror in `src/lib/api.ts` and types in `src/types/` if the UI needs it.

## Dependencies

- grammers crates are **git-pinned** in [Cargo.toml](Cargo.toml) (rev + `core2` patch). Do not bump casually; build breakage is common.
- Prefer `cargo check` from this directory after Rust changes.

## High-risk (minimal diffs)

- `session.rs` — AES-GCM session file + OS keyring
- `config.rs` — API credentials
- `telegram/auth.rs` — login / QR / 2FA flows

See [docs/security.md](../docs/security.md) for constraints.
