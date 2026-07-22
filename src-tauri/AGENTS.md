# Backend (`src-tauri/`) — agent notes

Rust Tauri 2 app: Telegram via ferogram, library in SQLite, media cache + stream protocol.

## Module map

| Module | Role |
|--------|------|
| `src/commands.rs` | Tauri IPC handlers |
| `src/lib.rs` | App setup, command registration, `stream` protocol |
| `src/db.rs` | SQLite (`library.db`) |
| `src/telegram/` | Client, auth, saved music sync, download |
| `src/session.rs` | Encrypted ferogram `SessionBackend` + keyring |
| `src/config.rs` | `TELEGRAM_API_ID` / `TELEGRAM_API_HASH` |
| `src/streaming.rs` | Range streaming for uncached tracks |
| `src/cache.rs` | On-disk media/thumbnail paths, size limit, TTL, eviction |
| `src/export.rs` | Copy tracks into system Downloads |
| `src/listen_stats.rs` | Listen qualification, aggregates, likeness |
| `src/state.rs` | Shared `AppState` |

## Adding a command

1. Implement in `commands.rs`.
2. Register in `lib.rs` `invoke_handler!`.
3. Mirror in `src/lib/api.ts` and types in `src/types/` if the UI needs it.

## Dependencies

- Pin `ferogram = "=0.6.4"` in [Cargo.toml](Cargo.toml). Do not bump casually; 0.x APIs may still shift.
- Do **not** enable ferogram `sqlite-session` (conflicts with our rusqlite usage pattern / dual session stores).
- Prefer `cargo check` from this directory after Rust changes.

## High-risk (minimal diffs)

- `session.rs` — AES-GCM session file + OS keyring (`SessionBackend`)
- `config.rs` — API credentials
- `telegram/auth.rs` — login / QR / 2FA flows

See [docs/security.md](../docs/security.md) for constraints.
