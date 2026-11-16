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
