# SoundGrammy — agent notes

Native GPUI music player over Telegram MTProto (ferogram). Local-first: SQLite owns the library and playlists; Telegram profile music is the remote source of truth.

## Layout

| Path | Role |
|---|---|
| `native/src/ui/` | GPUI views, account-scoped controller, presentation models and tests |
| `native/src/` | Rust services, SQLite, ferogram, session, streaming and audio |
| `native/gen/android/` | Native Activity, IME bridge, foreground playback service and JVM tests |
| `xtask/` | Rust xtask packaging, version and icon tooling |
| `native-app*.json` | Release/development identity, version and window configuration |
| `docs/` | Architecture and acceptance evidence |

## Commands

```sh
cargo run
cargo check --locked
cargo xtask check-dependencies
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --locked
cargo xtask package
cargo xtask android --debug
```

## Rules

- Keep Telegram, credentials, session crypto and playback ownership in their existing services. GPUI calls typed Rust functions; notification payloads stay typed.
- Keep auth/session/crypto diffs minimal (`session.rs`, `config.rs`, `telegram/auth.rs`).
- UI tasks must reject old-account results; ordered player commands must not overtake each other.
- Never pass Telegram document descriptors into the presentation model; use `Snapshot::for_display()`.
- Preserve release identities, database/session/cache paths and Android signing compatibility.
- Prefer existing modules over new abstractions. Cargo manages Rust dependencies and daily development.
- See [native/AGENTS.md](native/AGENTS.md) and [docs/architecture.md](docs/architecture.md).
