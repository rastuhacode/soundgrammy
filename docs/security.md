# Security

Agent-facing constraints for credentials and Telegram session material.

## Credentials

- At build time, [`build.rs`](../src-tauri/build.rs) embeds `TELEGRAM_API_ID` and `TELEGRAM_API_HASH` from the process environment or local `src-tauri/.env.local`. At runtime, [`config.rs`](../src-tauri/src/config.rs) prefers the environment over embedded values. Debug builds also load `.env.local` at runtime.
- `LASTFM_API_KEY` and `LASTFM_API_SECRET` follow the same resolution order and are optional for self-builds. Rust uses the secret to sign requests and opens the browser authorization URL with the application key.
- Never commit `.env`, `.env.local`, API hashes, or session files. Use `.env.example` as the template.
- Do not log credential values or put them in frontend code / analytics.

## Session at rest

- ferogram session (`PersistedSession`) is sealed with AES-256-GCM via a custom `SessionBackend` and written as `session.enc` under the app data dir (`session.rs`).
- The 256-bit key lives in the OS credential store under the effective application ID (`com.soundgrammy.app` for release, with a separate ID for development). On Android, Rust initializes the Keystore-backed credential store during [Tauri setup](../src-tauri/src/lib.rs) after Tao initializes the shared NDK context and before session repair.
- Android backup excludes the keyring SharedPreferences file and `session.enc`: Keystore keys do not survive uninstall or device transfer. A previously restored, unreadable session is cleared on startup without deleting the SQLite library.
- Session writes use a serialized, synced temporary file and atomic replacement. Local auth verifies ciphertext without contacting Telegram. Temporary keychain/read failures preserve the ciphertext; a requested new login quarantines confirmed corruption. Reads never create a missing encryption key. Development namespace migration copies legacy keys without changing release credentials.
- Never log session ciphertext, plaintext session snapshots, auth keys, or keyring secrets.
- Last.fm session keys use the same OS credential store under service `<effective application ID>.lastfm` (`com.soundgrammy.app.lastfm` for release), keyed by canonical Last.fm username. Temporary authorization tokens remain in backend memory.
- Treat edits to `session.rs`, `config.rs`, and `telegram/auth.rs` as high-risk: smallest possible diff, no drive-by refactors.

## Frontend surface

- Logged-in UI state is limited to the display fields in `AuthUser` and `SessionPayload`. Login inputs are transient.
- No MTProto auth keys, DC options, or encrypted session blobs in React state, localStorage, or logs.
- Last.fm IPC exposes only safe status, usernames, queue counts, timestamps, and fixed error messages. It never exposes application credentials, signatures, request bodies, auth URLs, temporary tokens, or session keys.
- Logout must clear app session state and rely on backend `logout` to drop the Telegram session.
- Offline UI may trust the local SQLite profile + `session.enc`; remote session revoke clears that local session only once the device reaches Telegram again (`refresh_auth` / `auth:revoked`).

## Review checklist (auth / crypto changes)

- [ ] No secrets in git, logs, or UI
- [ ] Session still loads/saves only via `session.rs`
- [ ] New IPC does not expose raw session or API hash to the webview
