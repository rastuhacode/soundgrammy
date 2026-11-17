<p align="center">
  <img src="native/icons/original/soundgrammy-original.svg" alt="SoundGrammy icon" width="128" height="128">
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

You can read more about philosophy around SoundGrammy in [Why I don’t use music streaming services](https://rastuharem.netlify.app/posts/why-i-cant-use-music-services/).

## Features

- 🔄**Automatic library sync** — add a track to your Telegram profile and it appears in SoundGrammy.
- 🎵**Full playback controls** — stream, cache, download, shuffle, and repeat tracks.
- 💿**Local playlists** — create, rename, reorder, delete, and cache playlists; export them for basic cross-device transfer.
- 🔐**Private listening statistics** — use the **Popular** and **Recent** smart playlists generated from local listening history.
- 📻**Last.fm scrobbling** — connect through Last.fm in your browser and upload qualified listens, including after temporary offline use.
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
| **Android** | (`apk`) |

The GPUI interface runs natively on desktop and Android. Android device acceptance is tracked in [the migration log](docs/gpui-migration.md). Linux is compiled in CI but is not an officially packaged release. An iOS application host is not configured; the existing iOS audio adapter remains available for future work.

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
- **Why can uncached tracks take longer to start?** SoundGrammy downloads the ranges needed by the native decoder. Startup and seeking depend on the container metadata and network speed. Caching a track first avoids network waits during playback.
- **How can I use this app if Telegram is blocked in my country?** - you can use VPN or connect to MTProto proxy (for ex. [tg-ws-proxy](https://github.com/Flowseal/tg-ws-proxy)).
- **I found a bug, how can I report it?** - enable logs in the settings and reproduce the problem. Copy logs if they exist and open an issue.

See the [current migration audit](docs/gpui-audit.md) and [measured performance comparison](docs/performance/report.md).

## Local development

### Prerequisites

- Install [Rust through rustup](https://www.rust-lang.org/tools/install); the repository selects Rust 1.98.1 and its Clippy/rustfmt components automatically.
- macOS: Xcode command-line tools; Windows: Visual Studio C++ build tools and Windows SDK
- Linux: Clang, pkg-config, Fontconfig, Wayland, XKB/X11, OpenSSL, Zstd, ALSA and Vulkan development/runtime packages
- Android: JDK 17, SDK platform 36, build-tools 35.0.0 and NDK 29.0.13846066; Rust target `aarch64-linux-android`
- Telegram requires third-party clients to use their own application API credentials. You can get yours `api_id` and `api_hash` from [my.telegram.org/apps](https://my.telegram.org/apps)

### Account-free checks

Debug checks, tests and the `--preview` fixture do not require Telegram credentials
or a copied `.env.local` file:

```sh
cargo check --locked
cargo test --workspace --locked
cargo xtask check-dependencies
cargo run --locked -- --preview
```

The preview uses temporary storage and does not start Telegram or audio services.
A normal app launch validates real credentials at runtime; release builds still
require them at build time. Follow the setup below to sign in to Telegram.

### Setup

```bash
cp native/.env.example native/.env.local
```

Add your Telegram application credentials to `native/.env.local`:

```dotenv
TELEGRAM_API_ID=your_api_id
TELEGRAM_API_HASH=your_api_hash
```

Last.fm scrobbling is optional for self-builds. To enable it, register a Last.fm API application and add its application credentials to the same file:

```dotenv
LASTFM_API_KEY=your_lastfm_api_key
LASTFM_API_SECRET=your_lastfm_shared_secret
```

The Last.fm user session is stored in the operating-system keychain. The application secret is embedded in configured desktop builds and should be treated as an application credential, not as a user credential.

Start the desktop app:

```bash
cargo run
```

The development app uses its own `com.soundgrammy.app.dev` identity, data directory,
cache and Telegram session, separate from the installed production app.
`native/.env.local` is ignored by Git, so each new worktree needs its own copy.

Build and install the Android development app:

```bash
rustup target add aarch64-linux-android
export ANDROID_HOME=/path/to/Android/sdk
cargo xtask android --debug
adb install -r native/gen/android/app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n com.soundgrammy.app.dev/com.soundgrammy.app.MainActivity
```

The task builds the Rust GPUI library, stages it for Gradle, packages the APK and runs JVM tests. The `.dev` application ID stays separate from the installed release. Rebuild after Rust changes. `cargo xtask android` creates a release APK; release signing uses `ANDROID_KEYSTORE_PATH`, `ANDROID_KEY_ALIAS`, and `ANDROID_KEY_PASSWORD`.

### Everyday Cargo workflow

```bash
cargo check --locked
cargo fmt --all
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask check-dependencies
```

To regenerate the native application icons from the source SVG:

```bash
cargo xtask icons
```

### Release build

```bash
cargo build --release --locked    # native release binary
cargo install cargo-packager --version 0.11.2 --locked # once, for installers
cargo xtask package               # macOS app/DMG; Windows EXE/MSI
cargo xtask package --target x86_64-apple-darwin
```

Installers are written under `dist/`. Development uses GPUI directly: there is no web server, browser runtime, React or JavaScript in the application. `cargo run -- --preview` launches an account-free fixture in temporary storage for visual checks.

The app uses GPUI Kit 0.10.0 with GPUI 0.3.12 on desktop and Android. See [dependency maintenance](docs/dependencies.md) for the renderer version and Android host patch.

GPUI describes the UI in Rust using flex layout, components and event handlers. It draws through the GPU instead of putting HTML into a WebView. Existing Telegram, database and audio services remain Rust services; UI calls are ordinary typed function calls. See [architecture](docs/architecture.md) and [migration verification](docs/gpui-migration.md).

### Safe release testing

A normal release build uses the production identity. Always set an isolated root
when testing an optimized build alongside your installed app:

```sh
SOUNDGRAMMY_DATA_DIR="$(mktemp -d /tmp/soundgrammy-test.XXXXXX)" cargo run --release --locked
```

The override must be absolute. Database, session file, cache, exports, preferences
and diagnostic logs all live below that root; credentials use a sandbox namespace.
Do not copy a production session or database into it. For an account-free visual
fixture use `cargo run --release --locked -- --preview` (also isolated).

`cargo run` compiles and starts the debug app; rerun it after edits. GUI/layout and image dependencies use optimization level 2 in development so trackpad scrolling and image preparation stay responsive; application code keeps normal debug information. The first build after this profile change rebuilds those dependencies. Rust has no
frontend server or hot reload here. `cargo check` checks types without linking;
`cargo build` produces the binary; `--release` enables optimization. `cargo fmt`
formats code; `cargo clippy` checks common mistakes; `cargo test --workspace` runs
both application and tooling tests. `--locked` prevents dependency resolution
from changing the committed root `Cargo.lock`. Use `cargo update -p <crate>` for
an intentional dependency update. Optional edit/restart: install `bacon` or
`cargo-watch`; neither is required. `cargo xtask help` lists installer, Android,
version and icon maintenance tasks. Bun, Node, package.json and TypeScript are
no longer part of this project's development workflow.

## Contributing and support

Use [GitHub Issues](https://github.com/rastuhacode/soundgrammy/issues) for reproducible, non-sensitive bug reports and feature requests. Contributions are welcome, especially for additional platform support.

Unless explicitly agreed otherwise, contributions submitted for inclusion in SoundGrammy must be licensed under `GPL-3.0-only`.

## License

Copyright © 2026 Rasten Remizov.

SoundGrammy is free software licensed under the [GNU General Public License version 3 only](LICENSE) (`GPL-3.0-only`). You may use, study, modify, and redistribute it under that license. Distributed modified versions and binaries must comply with the GPL's source-code and notice requirements. This license covers SoundGrammy itself; it does not grant rights to Telegram trademarks or to music and other third-party content accessed through the app.

## Legal and responsible use

SoundGrammy does not grant any license to music or other content available through a Telegram account. You are responsible for having the rights and permissions required to access, cache, copy, export, and play content, and for complying with copyright law, Telegram's terms, and applicable local law. Do not use SoundGrammy to infringe rights, evade access controls, scrape Telegram, train AI systems on Telegram data, spam, or perform actions prohibited by Telegram.

Use of the Telegram API is governed by Telegram's [API Terms of Service](https://core.telegram.org/api/terms), [Terms of Service](https://telegram.org/tos), [Content Licensing Terms](https://telegram.org/tos/content-licensing), and [Security Guidelines for Client Developers](https://core.telegram.org/mtproto/security_guidelines). These documents can change, so release maintainers should review them before every public release.

Optional scrobbling uses the [Last.fm API](https://www.last.fm/api) and is governed by its [API Terms of Service](https://www.last.fm/api/tos). Release maintainers must review those terms before distribution and contact Last.fm before commercial use.

The SoundGrammy name, artwork, and source code are independent of Telegram. The Telegram name and logo remain the property of their respective owner. References to Telegram describe compatibility and the remote service used by the app; they do not imply an official relationship.

### Native audio playback

Desktop playback uses the Rust audio engine in development and release builds.
Run `cargo run`; no playback environment variable is required. See [native audio](docs/native-audio.md)
for architecture, codec coverage, Linux dependencies, and verification gates.
The [architecture overview](docs/architecture.md) explains native playback ownership,
UI reattachment, and what persists across application restarts.
