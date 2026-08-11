<p align="center">
  <img src="src-tauri/icons/original/soundgrammy-original.svg" alt="SoundGrammy icon" width="128" height="128">
</p>

<h1 align="center">SoundGrammy</h1>

<p align="center"><strong>A local-first desktop player for the music saved to your Telegram profile.</strong></p>

<p align="center">
  <a href="https://github.com/rastuhacode/soundgrammy/releases"><img src="https://img.shields.io/github/v/release/rastuhacode/soundgrammy?display_name=tag&amp;sort=semver" alt="Latest release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--only-blue.svg" alt="License: GPL v3"></a>
</p>

<p align="center">
  <a href="https://github.com/rastuhacode/soundgrammy/releases">Download</a> ·
  <a href="https://github.com/rastuhacode/soundgrammy/issues/new">Report a bug</a>
</p>

> [!IMPORTANT]
> SoundGrammy is an independent, unofficial application that uses the Telegram API. It is not affiliated with, maintained by, sponsored by, approved by, or endorsed by Telegram or its team. Telegram is a trademark of its respective owner.

## Why SoundGrammy?

Telegram can store music on your profile, but it does not provide the library management and playback experience of a dedicated music player.

SoundGrammy was created to add the playback and organization tools that a music library needs and to eliminate manual synchronization between a local player and music saved on Telegram.

You can read more about philosophy around SoundGrammy in my [article]().

## Features

- 🔄**Automatic library sync** — add a track to your Telegram profile and it appears in SoundGrammy.
- 🎵**Full playback controls** — stream, cache, download, shuffle, and repeat tracks.
- 💿**Local playlists** — create, rename, reorder, delete, and cache playlists; export them for basic cross-device transfer.
- 🔐**Private listening statistics** — use the **Popular** and **Recent** smart playlists generated from local listening history.
- ✏️**Editable queue** — play next, add to the end, reorder tracks, and save the queue as a playlist.
- 💾**Downloads and exports** — download individual tracks or complete playlists, including an `M3U8` playlist file.
- 📝**Bulk actions** — add, cache, download, or manage many selected tracks at once.
- ✨**Thoughtful extras** — fullscreen playback (with artwork bounce), drag and drop, configurable caching, MTProto proxy support, smart shuffle and more.

### What SoundGrammy is NOT

- ❌**Not a music streaming service.** It does not provide recommendations or a music catalog; it only displays tracks that you have added to your Telegram profile.
- ❌**Not a replacement for the Telegram client.** SoundGrammy focuses on music playback and organization. Profile music is read-only in the app, so use an official Telegram client to change it.
- ❌**Not a cloud service.** SoundGrammy does not collect your personal information. This improves privacy, but means multi-device synchronization is intentionally limited to playlist import and export.

## Supported platforms

| Platform | Supported architectures |
| :--- | :--- |
| **macOS** | Apple Silicon (`arm64`) and Intel (`x86_64`) |
| **Windows** | 64-bit (`x86_64`) |

Linux, iOS, and Android are not officially supported. Tauri supports these platforms at a framework level, but SoundGrammy has not been tested or packaged for them. Contributions that add support for other platforms are welcome.

## How to use

1. Download the latest installer from [**GitHub Releases**](https://github.com/rastuhacode/soundgrammy/releases). Only install artifacts published by this repository, and verify the release tag before installing.
2. Install app and log in to your Telegram account.
3. Add music to your profile:
  - Desktop: hover track -> right click -> Save to... -> Profile.
  - Mobile: play track -> Add to Profile.
4. Sync "All tracks" by refreshing or avatar -> Radio Tower button.
5. Enjoy your music!

## FAQ

- **Why does app asks for my device password on initial launch on macOS?** - SoundGrammy uses keychain to cipher your session and macOS needs password for this operation.
- **Why some of my tracks play only when they fully cached, while others streams fine?** - SoundGrammy uses MSE streaming for uncached tracks playthrough. MSE works correctly with `mp3`, but can't properly work with `m4a`, `FLAC`. In this case you need to either manually cache it before playing or just wait a bit longer for song to fully cache.
- **How can I use this app if Telegram is blocked in my country?** - you can use VPN or connect to MTProto proxy (for ex. [tg-ws-proxy](https://github.com/Flowseal/tg-ws-proxy)).
- **I found a bug, how can I report it?** - enable logs in the settings and reproduce the problem. Copy logs if they exist and open an issue.

## Local development

### Prerequisites

- Install [Rust](https://www.rust-lang.org/learn/get-started)
- Install [Bun](https://bun.com/docs/installation)
- The [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system
- Telegram requires third-party clients to use their own application API credentials. You can get yours `api_id` and `api_hash` from [my.telegram.org/apps](https://my.telegram.org/apps)

### Setup

```bash
bun install
cp src-tauri/.env.example src-tauri/.env.local
```

Add your Telegram application credentials to `src-tauri/.env.local`:

```dotenv
TELEGRAM_API_ID=your_api_id
TELEGRAM_API_HASH=your_api_hash
```

Start the desktop app:

```bash
bun tauri:dev
```

### Quality checks

```bash
bun lint
bun typecheck
bun run test
cd src-tauri && cargo check
```

To regenerate every platform and web icon from the source SVG:

```bash
bun generate-icons
```

### Production build

```bash
bun tauri:build
```

## Contributing and support

Use [GitHub Issues](https://github.com/rastuhacode/soundgrammy/issues) for reproducible, non-sensitive bug reports and feature requests. Contributions are welcome, especially for additional platform support.

Unless explicitly agreed otherwise, contributions submitted for inclusion in SoundGrammy must be licensed under `GPL-3.0-only`.

## License

Copyright © 2026 Rasten Remizov.

SoundGrammy is free software licensed under the [GNU General Public License version 3 only](LICENSE) (`GPL-3.0-only`). You may use, study, modify, and redistribute it under that license. Distributed modified versions and binaries must comply with the GPL's source-code and notice requirements. This license covers SoundGrammy itself; it does not grant rights to Telegram trademarks or to music and other third-party content accessed through the app.

## Legal and responsible use

SoundGrammy does not grant any license to music or other content available through a Telegram account. You are responsible for having the rights and permissions required to access, cache, copy, export, and play content, and for complying with copyright law, Telegram's terms, and applicable local law. Do not use SoundGrammy to infringe rights, evade access controls, scrape Telegram, train AI systems on Telegram data, spam, or perform actions prohibited by Telegram.

Use of the Telegram API is governed by Telegram's [API Terms of Service](https://core.telegram.org/api/terms), [Terms of Service](https://telegram.org/tos), [Content Licensing Terms](https://telegram.org/tos/content-licensing), and [Security Guidelines for Client Developers](https://core.telegram.org/mtproto/security_guidelines). These documents can change, so release maintainers should review them before every public release.

The SoundGrammy name, artwork, and source code are independent of Telegram. The Telegram name and logo remain the property of their respective owner. References to Telegram describe compatibility and the remote service used by the app; they do not imply an official relationship.
