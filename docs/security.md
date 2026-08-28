# Security

Agent-facing constraints for credentials and Telegram session material.

## Credentials

- `TELEGRAM_API_ID` and `TELEGRAM_API_HASH` are embedded at compile time (`build.rs` → `option_env!`). Prefer process env (CI secrets); otherwise `src-tauri/.env.local` is loaded for local builds. Debug also loads `.env.local` at runtime for `tauri dev`.
- `LASTFM_API_KEY` and `LASTFM_API_SECRET` follow the same resolution order but are optional outside official releases. They remain backend-only; the application key appears only in the backend-opened authorization URL.
- Never commit `.env`, `.env.local`, API hashes, or session files. Use `.env.example` as the template.
- Do not log credential values or put them in frontend code / analytics.

## Session at rest

- ferogram session (`PersistedSession`) is sealed with AES-256-GCM via a custom `SessionBackend` and written as `session.enc` under the app data dir (`session.rs`).
- The 256-bit key lives in the OS keychain (`keyring`), service `com.soundgrammy.app`.
- Never log session ciphertext, plaintext session snapshots, auth keys, or keyring secrets.
- Last.fm session keys live only in the OS keychain under service `com.soundgrammy.app.lastfm`, keyed by canonical Last.fm username. Temporary authorization tokens remain in backend memory.
- Treat edits to `session.rs`, `config.rs`, and `telegram/auth.rs` as high-risk: smallest possible diff, no drive-by refactors.

## Frontend surface

- UI may hold display fields only (`AuthUser` / `SessionPayload`: name, username, ids).
- No MTProto auth keys, DC options, or encrypted session blobs in React state, localStorage, or logs.
- Last.fm IPC exposes only safe status, usernames, queue counts, timestamps, and fixed error messages. It never exposes application credentials, signatures, request bodies, auth URLs, temporary tokens, or session keys.
- Logout must clear app session state and rely on backend `logout` to drop the Telegram session.
- Offline UI may trust the local SQLite profile + `session.enc`; remote session revoke clears that local session only once the device reaches Telegram again (`refresh_auth` / `auth:revoked`).

## Review checklist (auth / crypto changes)

- [ ] No secrets in git, logs, or UI
- [ ] Session still loads/saves only via `session.rs`
- [ ] New IPC does not expose raw session or API hash to the webview
