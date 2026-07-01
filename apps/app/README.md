# SoundGrammy Desktop

Native desktop rewrite of the SoundGrammy web app, built with **Tauri 2**,
**React 19 + Vite + Tailwind CSS v4** on the frontend and a **Rust** core that
talks to Telegram over MTProto via [`grammers`](https://github.com/Lonami/grammers).

It logs you in with Telegram (QR or phone + 2FA), mirrors your saved profile
music into a local SQLite library, and plays tracks Telegram‑Desktop style:
each track (and its thumbnail) is downloaded and cached to disk on first play,
then served straight from the local cache via Tauri's asset protocol.

## Architecture

```
src/                 React frontend
  lib/api.ts         invoke() + event wrappers (replaces the old tRPC/HTTP layer)
  stores/            Zustand stores (player / library / playlists / session / …)
  components/        UI (auth, sidebar, playlist views, audio player)
  hooks/             cached thumbnail / avatar / sync-status hooks
src-tauri/src/       Rust core
  config.rs          Telegram api_id / api_hash
  error.rs           AppError (thiserror + Serialize)
  db.rs              rusqlite library (tracks / playlists / profile)
  session.rs         encrypted, persistent session (AES‑256‑GCM + OS keychain)
  cache.rs           content-addressed audio / thumb / avatar cache
  telegram/          client, auth (phone + QR), saved-music sync, download
  commands.rs        Tauri command surface
```

Key desktop shift vs. the web app: there is no HTTP boundary, no JWT cookies and
no session pool. A single persistent `grammers` client serves the one logged‑in
user; the session is restored from an encrypted blob on disk whose key lives in
the OS keychain.

## Telegram API credentials

The app needs a Telegram `api_id` / `api_hash` (create one at
<https://my.telegram.org>). As `grammers` recommends, they are embedded at build
time, but for local development they can also be provided at runtime:

```bash
export TELEGRAM_API_ID=123456
export TELEGRAM_API_HASH=your_api_hash
```

These are read by `src-tauri/src/config.rs` (runtime env first, then the value
embedded via `option_env!` at compile time).

## Development

From the repo root:

```bash
bun install
bun run app:dev      # turbo -> tauri dev (Vite dev server + Rust shell)
```

or from `apps/app`:

```bash
bun run tauri:dev
```

## Production build

```bash
bun run app:build    # turbo -> tauri build
# or, in apps/app:
bun run tauri:build
```

## Notes

- QR login is the primary flow (works without SMS, required for Russian
  numbers). Phone + code + 2FA is available as a fallback.
- Saved‑music sync uses paginated `users.GetSavedMusic` with Telegram's XOR list
  hash for cheap change detection; unchanged libraries short‑circuit.
- Cached files live under the app cache dir; the asset‑protocol scope is limited
  to that directory.
