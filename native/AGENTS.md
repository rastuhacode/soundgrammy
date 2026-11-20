# Native application — agent notes

`runtime.rs` owns Tokio, typed notifications, native paths and application lifetime. `ui/` owns GPUI presentation. `commands*` are ordinary Rust service entry points.

- `audio/` owns playback, queue/shuffle policy, listening accounting and OS media controls.
- `db*` owns account-scoped SQLite data and positional playlist mutations.
- `telegram/` owns MTProto, authentication, profile music reconciliation and downloads.
- `session.rs` owns encrypted ferogram session persistence and platform credentials.
- `cache*`, `streaming*`, `export.rs`, `playlist_recipe.rs`, `lastfm/` retain their domain ownership.
- Android keeps the process-owned core alive through Activity recreation; JNI media callbacks reach audio directly.

Run `cargo check`, `cargo test`, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings` from this directory. The repository is a Cargo workspace; run these commands from the root as well.

Do not enable ferogram `sqlite-session`. Keep changes minimal in `session.rs`, `config.rs`, and `telegram/auth.rs`. Treat UI task epochs, database account generations, queue revisions and stream cancellation as separate guards. Never reintroduce a second playback owner into the UI.

## UI ownership

- `ui/app.rs` composes retained library, playback, sign-in and settings entities.
- Keep feature behavior under `ui/library/`, `ui/player/`, `ui/login.rs`, `ui/settings.rs`,
  `ui/fullscreen.rs` and `ui/panels.rs`; the module index is bootstrap/configuration.
- Emit `runtime::Event` variants. Do not reintroduce string notification names or `Any` payloads.
- Startup disk work belongs to `ui/startup.rs`'s blocking worker. UI preference/log writes
  use `AppHost::persist_setting`; the runtime writer coalesces and flushes them.
- Schedule artwork/timer/native-transition work from intent/result handling through
  `ui/effects.rs`, rather than the root render method. Views may reconcile their own
  native input/focus/layout state.
- Cached layers observe their feature model and receive explicit presentation inputs.
  Add a new shared dependency to those inputs when changing what a cached feature reads.
- Run `cargo xtask check-dependencies` after dependency edits; see `docs/dependencies.md`.
