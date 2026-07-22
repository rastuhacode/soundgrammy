# Security

Agent-facing constraints for credentials and Telegram session material.

## Credentials

- `TELEGRAM_API_ID` and `TELEGRAM_API_HASH` come from the environment (or compile-time embed). Dev may use `.env.local` (loaded only in debug).
- Never commit `.env`, `.env.local`, API hashes, or session files.
- Do not log credential values or put them in frontend code / analytics.

## Session at rest

- ferogram session (`PersistedSession`) is sealed with AES-256-GCM via a custom `SessionBackend` and written as `session.enc` under the app data dir (`session.rs`).
- The 256-bit key lives in the OS keychain (`keyring`), service `com.soundgrammy.app`.
- Legacy grammers-shaped JSON in `session.enc` is deleted on load (clean cut after the ferogram migration); users re-login once.
- Never log session ciphertext, plaintext session snapshots, auth keys, or keyring secrets.
- Treat edits to `session.rs`, `config.rs`, and `telegram/auth.rs` as high-risk: smallest possible diff, no drive-by refactors.

## Frontend surface

- UI may hold display fields only (`AuthUser` / `SessionPayload`: name, username, ids).
- No MTProto auth keys, DC options, or encrypted session blobs in React state, localStorage, or logs.
- Logout must clear app session state and rely on backend `logout` to drop the Telegram session.

## Review checklist (auth / crypto changes)

- [ ] No secrets in git, logs, or UI
- [ ] Session still loads/saves only via `session.rs`
- [ ] New IPC does not expose raw session or API hash to the webview
