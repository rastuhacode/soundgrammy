# Security

Agent-facing constraints for credentials and Telegram session material.

## Credentials

- `TELEGRAM_API_ID` and `TELEGRAM_API_HASH` come from the environment (or compile-time embed). Dev may use `.env.local` (loaded only in debug).
- Never commit `.env`, `.env.local`, API hashes, or session files.
- Do not log credential values or put them in frontend code / analytics.

## Session at rest

- grammers session is snapshotted, sealed with AES-256-GCM, written as `session.enc` under the app data dir (`session.rs`).
- The 256-bit key lives in the OS keychain (`keyring`), service `com.soundgrammy.app`.
- Never log session ciphertext, plaintext `SessionData`, auth keys, or keyring secrets.
- Treat edits to `session.rs`, `config.rs`, and `telegram/auth.rs` as high-risk: smallest possible diff, no drive-by refactors.

## Frontend surface

- UI may hold display fields only (`AuthUser` / `SessionPayload`: name, username, ids).
- No MTProto auth keys, DC options, or encrypted session blobs in React state, localStorage, or logs.
- Logout must clear app session state and rely on backend `logout` to drop the Telegram session.
- Offline UI may trust the local SQLite profile + `session.enc`; remote session revoke clears that local session only once the device reaches Telegram again (`refresh_auth` / `auth:revoked`).

## Review checklist (auth / crypto changes)

- [ ] No secrets in git, logs, or UI
- [ ] Session still loads/saves only via `session.rs`
- [ ] New IPC does not expose raw session or API hash to the webview
