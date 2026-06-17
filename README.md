# SoundGrammy

Personal music library from your Telegram profile.

## Local development

```bash
bun dev
```

Open [http://localhost:3000](http://localhost:3000).

### Env vars (`apps/web/.env.local`)

```env
TELEGRAM_API_ID=12345678
TELEGRAM_API_HASH=your_api_hash
JWT_SECRET=long-random-string
MTPROTO_SESSION_SECRET=long-random-string  # optional, defaults to JWT_SECRET
```

Get `TELEGRAM_API_ID` and `TELEGRAM_API_HASH` from [my.telegram.org](https://my.telegram.org).

### Auth

Sign in with your Telegram phone number (MTProto). After login, use **Sync profile music** to import songs pinned to your Telegram profile.
