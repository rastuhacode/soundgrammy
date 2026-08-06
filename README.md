# SoundGrammy

> [!IMPORTANT]
> SoundGrammy is an independent, unofficial application that uses the Telegram API. It is not affiliated with, maintained by, sponsored by, approved by, or endorsed by Telegram or its team. Telegram is a trademark of its respective owner.

## Philosophy

SoundGrammy is a local-first desktop player for the music saved in your Telegram profile. It adds missing essential features, which Telegram doesn't have out of the box currently.

I've created it as I was tired of manual syncs between my local player and my online Telegram's profile music. You can read more about this in my [article]().

## Features

- Automatic sync of your library: add track to profile to Telegram and it will appear in you SoundGrammy library.
- Basic Telegram player features: stream, cache and download tracks, shuffle, repeat, etc.
- Playlists: create, rename, reorder, delete, cache local playlists out of the box. Cross-device playlist sync via export feature.
- Listening statisctics (fully local): use Popular and Recent smart playlists built from listening history.
- Queue: you can view and edit your queue while playing it: "Play next", "Add to end", reorder, etc.
- Download tracks or playlists, including `M3U8` playlist files.
- Bulk operations: you can make most of the operations on multiple tracks, e.g add 100 selected tracks to playlist or cache them.
- Lots of "Nice to have" features like: fullscreen mode (with bounce!), cache configuration, drag & drop, MTProto proxy and many more!

What SoundGrammy is NOT:

- It is not a music streaming service: you won't find "Recommended for you" playlists and anything like this. SoundGrammy can only view tracks you added to your profile.
- It is not a Telegram client replacer - SoundGrammy only redesigns and adds missing features for the music playthrough. Additionally, your profile music is immutable inside app, so you would need original client to modify your profile music.
- It is not a cloud service: SoundGrammy doesn't collect any of your information. This is good for your privacy, but could be less comfort when using multiple devices. While SoundGrammy support basic cross-device sync, still keep it in mind.

## Supported platforms

The current supported platforms are:

| Platform | Architectures |
| --- | --- |
| macOS | Apple Silicon and Intel |
| Windows | x86-64 |

While Tauri technically supports all platforms like IOS, Android and Linux the app doesn't oficially supports it. Android support may be added in the future, other platforms I just can't test on my devices. I would gladly accept other platforms support via contributions.

## Security

MTProto and Telegram credentials are stored encrypted on your device's keychain and used ONLY to authorize in Telegram and get saved music.

For more detail, read [docs/security.md](docs/security.md).

Do not include phone numbers, login codes, two-step verification passwords, API hashes, session files, database files, or private media in public bug reports.

## Install

Prebuilt release artifacts, when available, are published on the [GitHub Releases page](https://github.com/rastuhacode/soundgrammy/releases). Download only artifacts published by this repository and verify the release tag before installing.

## Contributing and support

Use [GitHub Issues](https://github.com/rastuhacode/soundgrammy/issues) for reproducible, non-sensitive bug reports and feature requests. Unless explicitly agreed otherwise, contributions submitted for inclusion in SoundGrammy must be licensed under GPL-3.0-only.

## Local development

### Prerequisites

- Install [Rust](https://rust-lang.org/learn/get-started/)
- Insatll [Bun](https://bun.com/)
- The [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system
- A Telegram account and an application `api_id`/`api_hash` from [my.telegram.org/apps](https://my.telegram.org/apps)

Telegram requires third-party clients to use their own application API credentials. Never commit your `api_hash`, local environment files, or Telegram session data.

### Setup

```bash
bun install
cp src-tauri/.env.example src-tauri/.env.local
```

Fill in `src-tauri/.env.local`:

```dotenv
TELEGRAM_API_ID=your_api_id
TELEGRAM_API_HASH=your_api_hash
```

Start the desktop app:

```bash
bun tauri:dev
```

Release builds embed the application credentials supplied through `TELEGRAM_API_ID` and `TELEGRAM_API_HASH`, either from `src-tauri/.env.local` or the build environment. CI stores them as GitHub Actions secrets. These are the application's Telegram API credentials, not an end user's login session.

### Checks

```bash
bun lint
bun typecheck
bun run test
cd src-tauri && cargo check
```

Generate all platform and web icons from the source SVG:

```bash
bun generate-icons
```

### Build

To build from source, follow the development setup and run:

```bash
bun tauri:build
```

## License

Copyright © 2026 Rasten Remizov.

SoundGrammy is free software licensed under the [GNU General Public License version 3 only](LICENSE) (`GPL-3.0-only`). You may use, study, modify, and redistribute it under that license. Distributed modified versions and binaries must comply with the GPL's source-code and notice requirements. This license covers SoundGrammy itself; it does not grant rights to Telegram trademarks or to music and other third-party content accessed through the app.

## Legal and responsible use

Use of SoundGrammy does not grant any license to music or other content available through a Telegram account. You are responsible for having the rights and permissions required to access, cache, copy, export, and play content, and for complying with copyright law, Telegram's terms, and applicable local law. Do not use SoundGrammy to infringe rights, evade access controls, scrape Telegram, train AI systems on Telegram data, spam, or perform actions prohibited by Telegram.

Use of the Telegram API is governed by Telegram's [API Terms of Service](https://core.telegram.org/api/terms), [Terms of Service](https://telegram.org/tos), [Content Licensing Terms](https://telegram.org/tos/content-licensing), and [Security Guidelines for Client Developers](https://core.telegram.org/mtproto/security_guidelines). These documents can change, so release maintainers should review them again before every public release.

The SoundGrammy name, artwork, and source code are independent from Telegram. The Telegram name and logo remain the property of their respective owner. References to Telegram describe compatibility and the remote service used by the app; they do not imply an official relationship.
