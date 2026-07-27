# SoundGrammy

App which combines music streaming service with telegram providing features like playlists, smart shuffle, etc with local-first telegram mindset.

## Local development

1. Install [Rust](https://rust-lang.org/learn/get-started/)
2. Install [bun](https://bun.com/)
3. Install dependencies
```bash
bun i
```
4. Create Telegram API credentials at [my.telegram.org/apps](https://my.telegram.org/apps), then:
```bash
cp src-tauri/.env.example src-tauri/.env.local
# fill TELEGRAM_API_ID and TELEGRAM_API_HASH
```
5. Run dev server via [tauri](https://tauri.app/)
```bash
bun tauri:dev
```

Release builds (`bun tauri:build`) embed the same credentials from `.env.local`, or from `TELEGRAM_API_ID` / `TELEGRAM_API_HASH` in the environment (for CI).
