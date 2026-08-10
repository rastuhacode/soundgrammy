<p align="center">
  <img src="src-tauri/icons/original/soundgrammy-original.svg" alt="SoundGrammy icon" width="128" height="128">
</p>

<h1 align="center">SoundGrammy</h1>

<p align="center"><strong>A local-first desktop player for the music saved to your Telegram profile.</strong></p>

<p align="center">
  <a href="https://github.com/rastuhacode/soundgrammy/releases"><img src="https://img.shields.io/github/v/release/rastuhacode/soundgrammy?display_name=tag&amp;sort=semver" alt="Latest release"></a>
  <a href="https://github.com/rastuhacode/soundgrammy/actions/workflows/ci.yml"><img src="https://github.com/rastuhacode/soundgrammy/actions/workflows/ci.yml/badge.svg" alt="CI status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--only-blue.svg" alt="License: GPL v3"></a>
</p>

<p align="center">
  <a href="https://github.com/rastuhacode/soundgrammy/releases">Download</a> ·
  <a href="https://github.com/rastuhacode/soundgrammy/issues/new">Report a bug</a> ·
  <a href="https://github.com/rastuhacode/soundgrammy/issues/new">Request a feature</a>
</p>

> [!IMPORTANT]
> SoundGrammy is an independent, unofficial application that uses the Telegram API. It is not affiliated with, maintained by, sponsored by, approved by, or endorsed by Telegram or its team. Telegram is a trademark of its respective owner.

## Why SoundGrammy?

Telegram can store music on your profile, but it does not provide the library management and playback experience of a dedicated music player. SoundGrammy bridges that gap: **Telegram remains the remote source of truth**, while your library, playlists, cache, and listening history stay on your device.

It was created to eliminate manual synchronization between a local player and music saved on Telegram—and to add the playback and organization tools that a music library needs.

## Features

- **Automatic library sync** — add a track to your Telegram profile and it appears in SoundGrammy.
- **Full playback controls** — stream, cache, download, shuffle, and repeat tracks.
- **Local playlists** — create, rename, reorder, delete, and cache playlists; export them for basic cross-device transfer.
- **Private listening statistics** — use the **Popular** and **Recent** smart playlists generated from local listening history.
- **Editable queue** — play next, add to the end, reorder tracks, and save the queue as a playlist.
- **Downloads and exports** — download individual tracks or complete playlists, including an `M3U8` playlist file.
- **Bulk actions** — add, cache, download, or manage many selected tracks at once.
- **Thoughtful extras** — fullscreen playback (with artwork bounce), drag and drop, configurable caching, MTProto proxy support, and more.

### What SoundGrammy is not

- **Not a music streaming service.** It does not provide recommendations or a music catalog; it only displays tracks that you have added to your Telegram profile.
- **Not a replacement for the Telegram client.** SoundGrammy focuses on music playback and organization. Profile music is read-only in the app, so use an official Telegram client to change it.
- **Not a cloud service.** SoundGrammy does not collect your personal information. This improves privacy, but means multi-device synchronization is intentionally limited to playlist import and export.

## Supported platforms

| Platform | Supported architectures |
| :--- | :--- |
| **macOS** | Apple Silicon (`arm64`) and Intel (`x86_64`) |
| **Windows** | 64-bit (`x86_64`) |

Linux, iOS, and Android are not officially supported. Tauri supports these platforms at a framework level, but SoundGrammy has not been tested or packaged for them. Contributions that add support for other platforms are welcome.

## Installation

Download the latest installer from [**GitHub Releases**](https://github.com/rastuhacode/soundgrammy/releases). Only install artifacts published by this repository, and verify the release tag before installing.

## Privacy and security

SoundGrammy is local-first. Your Telegram session, local library, playlists, cache metadata, and listening history remain on your device. MTProto session data is encrypted with AES-256-GCM, while the encryption key is kept in your operating system's keychain. The session is used only to authenticate with Telegram and access your saved music.

For implementation details and the security review checklist, read the [security documentation](docs/security.md).

> [!WARNING]
> Never include phone numbers, login codes, two-step verification passwords, API hashes, session files, database files, or private media in a public bug report.

## Local development

SoundGrammy uses [React](https://react.dev/), [TypeScript](https://www.typescriptlang.org/), [Tauri 2](https://v2.tauri.app/), [Rust](https://www.rust-lang.org/), and [SQLite](https://www.sqlite.org/). Telegram connectivity is provided through MTProto.

### Prerequisites

- [Rust](https://www.rust-lang.org/learn/get-started)
- [Bun](https://bun.com/docs/installation) 1.3 or newer
- The [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system
- A Telegram account and your own application `api_id` and `api_hash` from [my.telegram.org/apps](https://my.telegram.org/apps)

Telegram requires third-party clients to use their own application API credentials. **Never commit** your `api_hash`, local environment files, or Telegram session data.

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

Release builds embed the application credentials supplied through `TELEGRAM_API_ID` and `TELEGRAM_API_HASH`, either from `src-tauri/.env.local` or the build environment. CI stores them as GitHub Actions secrets. These are the **application's API credentials**, not an end user's login session.

### Quality checks

```bash
bun lint
bun typecheck
bun test
cd src-tauri && cargo check
```

To regenerate every platform and web icon from the source SVG:

```bash
bun generate-icons
```

### Production build

After completing the development setup, run:

```bash
bun tauri:build
```

## Architecture and documentation

The frontend communicates with the Rust backend exclusively through Tauri commands and events. The backend owns Telegram access, session security, SQLite persistence, and media streaming.

- [Architecture and data flow](docs/architecture.md)
- [Streaming transport](docs/streaming.md)
- [Listening statistics](docs/listen-statistics.md)
- [Security model](docs/security.md)
- [Release process](docs/releasing.md)

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
