# GPUI migration implementation and acceptance

> Historical migration evidence. The [current audit](gpui-audit.md) supersedes development commands, baseline references and test counts below.

Branch: `codex/gpui-migration`, based on `develop` at `6238384` (8 commits ahead of local `main`).

The application, build tools and release workflows now use native GPUI. The Tauri/WebView/React implementation has been removed from the working tree; the previous implementation and its test suites remain in the baseline commit. This log distinguishes implemented behavior from acceptance evidence. Automated checks and fixture inspection do not certify identical rendering or real-account behavior on every platform.

## Implemented scope

- QR, phone, code and 2FA sign-in; local session bootstrap; offline library use and reconnect backoff.
- Library indexing and full reindex confirmation; local playlists, likes, smart playlists, search, multiple sort columns, positional selection and drag ordering with duplicate memberships preserved.
- Native playback, buffered seeking, volume/mute, shuffle modes, repeat, editable queue, collapsible queue sections and saving the queue as a playlist.
- Track/playlist cache jobs, downloads, playlist recipes and import preview/name override, detailed track metadata and native file dialogs.
- Cache/listening-statistics settings, MTProto proxy settings, Last.fm browser authorization/scrobbling and both pending-scrobble disconnect policies, persisted opt-in diagnostics with copying/clearing.
- Fullscreen artwork/palette, bounce controls and native display wake ownership, queue/volume access, inactivity controls and keyboard/Back dismissal. Compact navigation, native focus traps, contextual menus and desktop application menus/shortcuts use the same GPUI views.
- One-time read-only import of previous WebView preferences and diagnostic history. Chromium recovery opens a temporary copy; original storage is never modified by the importer.
- Android NativeActivity/IME host, foreground playback, MediaSession, keyring initialization, Downloads and task-removal ownership. The process-owned core survives Activity/view replacement.
- Bun development, build, icon generation, version synchronization and packaging scripts; desktop/Android CI and release workflows; macOS app/DMG and Windows NSIS/MSI configurations. The Windows MSI UpgradeCode preserves the existing Tauri release lineage.

## Preserved boundaries

`native/src/runtime.rs` provides typed notifications, main-thread dispatch, weak handles, stable native paths and application lifetime. `native/src/ui/` owns GPUI presentation and calls the existing typed Rust service entry points. There is one playback owner in `audio/`; session encryption and authentication algorithms are preserved (`session.rs` is unchanged from the baseline).

UI tasks reject obsolete account epochs. Database mutations check account generations; player commands use an ordered account-scoped port; queue edits retain revision/position checks. `Snapshot::for_display()` strips Telegram document descriptors from both queue and base entries before presentation.

Release/development identifiers remain `com.soundgrammy.app` and `com.soundgrammy.app.dev`. Database, session, cache and Android signing identities retain their existing paths/contracts. Local application credentials are ignored build inputs and are not part of this patch.

## Dependencies

- GPUI Kit `0.8.1`, GPUI `0.3.10`, committed Cargo lockfile.
- Android host: `longbridge/gpui-mobile` revision `7548ebc4b24f00494af9976af8852c7163ded9ce`, Apache-2.0. Bundled Java host sources carry their notice/license; the Rust dependency is pinned by revision.
- Native packager `@crabnebula/packager` `0.11.2`; retained Windows template/provenance in `native/packaging/`.

## Verification on this machine

Baseline before migration: 271 Rust tests passed, 1 ignored; 388 frontend tests passed across 66 suites. The [test migration inventory](gpui-test-migration.md) maps those suites to native owners and explains which framework-specific layers were retired. This is a coverage map, not a claim that every prior test has a line-for-line equivalent.

Final checks cover Rust formatting, Clippy with warnings denied, native service/model/GPUI regressions, 5 Bun build-contract tests, TypeScript tooling checks, version synchronization and generated icon validation. Build/test counts and artifact checks below refer to the final successful local runs:

- Native Rust tests: **359 passed, 1 pre-existing ignored**, including 55 production GPUI view interaction/layout checks at desktop and compact sizes.
- Android arm64 Rust compilation, Gradle debug APK packaging and **3 JVM tests** passed. APK signature verification passed. The native library exports NativeActivity, Android main, IME and Back JNI entry points.
- Android release-host smoke check: R8/minification, release lint and **3 release-variant JVM tests** passed against the staged native library. The R8 mapping retains `MainActivity.background`, called from Rust. This host check used the staged debug Rust library and does not represent a signed release APK build.
- Apple Silicon macOS release app and DMG built. Strict bundle signature verification and DMG checksum verification passed. Local macOS artifacts use ad-hoc signing and are not notarized.
- Live macOS comparisons used the installed `/Applications/SoundGrammy.app` 3.5.0 as the reference, including its actual 460-track library and cached artwork. An isolated display fixture reproduced the same library, playlist counts, artwork, queue position and preferences without opening Telegram or modifying the account. Desktop and narrow-window library/navigation, player, queue, settings, playlist dialogs and fullscreen were inspected as rendered images. The rebuilt packaged migration also loaded the real account after the user approved its Keychain prompt. A cached track played with advancing progress, then paused; its title/artwork and 460-entry queue remained visible. This confirms native playback state and controls, not independently measured audible output. See [visual comparison evidence](gpui-visual-parity.md).
- Native regression coverage includes progress-only snapshot merging (so playback ticks do not erase the current track/queue), duplicate-preserving queue save scopes, queued download results, native sign-in/code input, volume popover pointer travel/exact entry, compact player/queue layering and minimum-width sidebar layout.
- Fullscreen inactivity was checked live: artwork remains accessible, stationary pointer refreshes do not restart the hide timer, keyboard input restores controls and Escape returns to the library.

Outputs (ignored): `dist/darwin/release/SoundGrammy.app`, `dist/darwin/release/SoundGrammy_3.5.0_aarch64.dmg`, and `native/gen/android/app/build/outputs/apk/debug/app-debug.apk`. The APK uses the isolated development identifier. Cargo outputs for this session are in `/private/tmp/soundgrammy-gpui-preflight/target`; verification logs use `/private/tmp/soundgrammy-*.log`.

## Acceptance still requiring another environment

The user explicitly deferred Android hardware checks: “I will check android version later”. No Android device/emulator was available. IME composition, notification/lock-screen controls, audible/background playback, interruptions, Activity recreation and task removal must be exercised on a device. A signed release APK/update was not installed locally.

Windows, Linux and Intel macOS workflows are configured, but were not executed in their target environments during this session. Installer upgrades, OS media integrations and visual parity there still need platform acceptance. Remote CI/release workflows were not dispatched and nothing was published.

A complete fresh real-account sign-in/2FA/QR and streaming/export/scrobbling walkthrough are not claimed by this evidence. Live screenshot comparison is human visual inspection, not a cross-platform pixel-diff certification. See [release acceptance](releasing.md#workflow-and-acceptance) before distributing a release.
