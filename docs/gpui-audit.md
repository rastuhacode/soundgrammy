# GPUI migration audit — 2026-11-14

## Shared drawer backdrop — 2026-11-18

The drawer now slides over the same dimmed backdrop on every platform. Its
opaque panel and shadow separate it from the library without sampling the
scene. Fullscreen uses the existing prepared artwork layers on macOS as well
as other platforms. The Apple renderer override and its Metal-only fixture
metrics are gone; the frame callback probe remains. The previous renderer
measurements below apply to the earlier build.

The macOS build, 403 app tests, 5 xtask tests, and account-free drawer and
fullscreen fixtures passed. Android arm64 produced a signed debug APK; the
three JVM tests passed. Windows, Linux and Android device rendering were not
exercised.

## Rust dependency upgrade — 2026-11-18

Updated the workspace manifests and lockfile, including GPUI Kit 0.10.0,
GPUI 0.3.12, ferogram 0.6.7, Tokio 1.54.1, Rusqlite 0.43.0, and JNI 0.22.4.
At that point the Android host and Apple renderer still had local overrides.
The [dependency notes](dependencies.md) explain the current graph, including
why legacy `block` still appears in the lockfile.

The upgrade also uses Kit's tab focus handling and replaces the old private
macOS cursor selector with AppKit's public API. An account-free macOS preview
exercised scrolling, the navigation drawer, and fullscreen. The scrolling
fixture sustained 120 frame callbacks per second on the tested display.
The fullscreen fixture completed after the cursor fix and recorded Metal
presentation timestamps.

The credential-free workspace run passed 403 application tests and 5 xtask
tests, with one existing ignored test. Cargo reported zero future
incompatibility warnings. The 24 upstream Ferogram sender tests also passed.
The macOS build, desktop and Android arm64 Clippy, and Android debug APK
build passed. All three Android JVM tests passed. Windows and Linux app runs,
Android device behavior, and live Telegram sessions remain to be exercised.

## Architecture and DX follow-up — 2026-11-17

This follow-up starts from `5136e94` on `t3/gpui-migration-audit`. The current
layout and ownership rules are documented in [architecture.md](architecture.md).
Earlier verification counts below remain historical evidence.

| Finding | Implemented correction |
|---|---|
| UI composition, state and feature behavior concentrated in large module files | Split account, library, player, settings, panels and fullscreen behavior into feature modules. Library/playback are observable entities; retained sign-in and settings entities own their native form state. |
| Cached views subscribed to every shell update | Observe feature models and pass explicit shared presentation inputs, including panel identity, selection, artwork and progress. Playback ticks update the seek control without redrawing unrelated track rows. |
| Startup and preference/log persistence could block GPUI | Run storage initialization and preference import on a blocking worker. A dedicated settings worker coalesces values per key over a bounded 150 ms interval and flushes on orderly shutdown. |
| Render methods initiated asynchronous and native transition work | Prepare artwork, loading timers and fullscreen transitions from intent/result effects. Account reset schedules those effects immediately. |
| Notifications relied on string names and payload downcasts | Use an exhaustive typed event enum throughout producers and consumers. |
| Moving generation reads off the UI could reorder or admit obsolete intent | Retain the serialized player command port and mutation guard, with an additional account-lifetime token alongside epoch and database-generation checks. |
| Optional hydration failures could hide useful library state | Preserve loaded tracks and last-known playlists/cache/statistics, expose recoverable errors, and retain positional selection only for identical ordered membership. |
| Custom controls duplicated available Kit behavior | Use Kit's sidebar resize handle, Accordion compound parts and RadioGroup semantics. Retain application-owned tab keyboard navigation because Kit 0.8.1 does not provide the complete behavior. |
| Theme literals and dependency maintenance were scattered | Centralize semantic colors, pin Rust 1.98.1, add a dependency-policy check to CI, and document patch provenance and removal criteria. Patch the legacy Metal `block` dependency's foreign type/ABI warning and exercise its capture lifetime on macOS. |
| Debug checks required account credentials | Permit credential-free debug checks/tests/preview; normal launches still validate credentials and release builds retain their credential requirement. |

Verification of this follow-up on Apple Silicon macOS:

- `cargo test --workspace --locked --offline --quiet` with Telegram and Last.fm
  credential variables explicitly empty: **404 application tests and 5 xtask
  tests passed**, with one pre-existing ignored application test. Coverage includes
  hydration/duplicate selection, settings flush under SQLite contention, sidebar
  dragging, account reset/fullscreen, progress rendering and legacy block lifetime.
- Workspace Clippy with all targets and warnings denied, formatting, dependency
  policy, version synchronization, and `git diff --check` passed.
- `cargo build --locked --offline` passed for the macOS debug application.
- Android arm64 `cargo check` with `--no-default-features --features mobile
  --locked --offline` passed with NDK 29.0.13846066.

This follow-up did not launch an app against a live account, inspect a rendered
window, build an APK, or run Android device/Windows/Linux/Intel macOS acceptance.
Those platform and account checks remain outstanding. See
[dependencies.md](dependencies.md) for the framework upgrade procedure.

## Result and baseline

The migrated app implements the original application's major desktop flows in Rust, GPUI Kit 0.8.1 and GPUI 0.3.10. This audit fixes specific missing behavior; it does **not** certify pixel-identical rendering, live Telegram account acceptance or every target platform.

The worktree initially pointed to `6238384`, with no migration committed. The migration existed as tracked edits plus seven untracked UI modules/documentation in the earlier `codex/gpui-migration` worktree. Those source changes were recovered into this worktree without altering the earlier worktree or copying ignored credentials, databases, caches or build outputs. `develop` at **745ef82** is the original-app reference. Its newer artwork/bounce changes were absent from the recovered migration.

Historical implementation/interaction evidence remains in [gpui-migration.md](gpui-migration.md), [gpui-visual-parity.md](gpui-visual-parity.md) and [gpui-test-migration.md](gpui-test-migration.md). Their commands/test counts predate this audit.

## Changes made

| Gap | Correction |
|---|---|
| Original app's database was newer than the migration | Ported `develop`'s schema v3 migration, three frequency-band columns and analysis algorithm v2. Older cached profiles regenerate through the version check. This also prevents the migration from rejecting a v3 library as a newer application database. Only disposable databases were opened during this audit. |
| Fullscreen used the retired manual bounce model | Ported bass/mid/treble transients, onset density, track adaptation, bass-first motion, upper-band handoff, attack/release, profile crossfade, pause release and seek resets. Removed Strength/Balance/Smoothness controls while retaining the saved enabled preference. Missing analysis causes no invented beats. |
| Fullscreen colors lagged `develop` | Added shadow-tint and repeated-accent recovery, dark monochrome completion, ambient/active/peak color ranges and frequency-band palette motion. Image sampling and layer creation run once per cover on the blocking executor. Animation samples prepared lanes and composites cached textures. Native GPU compositing approximates the original CSS gradients/blending; cross-platform pixel identity is not claimed. |
| Statistics opt-out did not hide smart playlists | Read the persisted setting at startup; hide Popular/Recent while disabled and return an active smart playlist to All tracks. Existing history stays intact. |
| Sidebar recency used sync time and removed-track history | Use the newest library track creation date; smart playlist recency only considers tracks still in the library. |
| Missing artist/duration sorted differently | Match the original nullable sort behavior in ascending and descending order, retaining positional identity for duplicate memberships. |
| Accessibility disabled the cache and dropped layout constraints | Preserve sizing in the uncached layer wrapper and explicitly size the player bar. This prevents the player from shrinking when assistive tooling connects. |
| Typography/artwork fallbacks differed | Match medium title weights, player artist sizing, Music placeholders and cached/uncached thumbnail borders. |
| Transfer progress never reached thumbnail borders | Consume the typed `download:progress` notification, paint the rounded progress border and clear it after completion, cache changes, removal or account reset. |
| Preview/test databases were shared | Give each account-free preview its own temporary directory. Parallel views no longer race while upgrading one shared SQLite database. |
| Release testing could select production storage | Add an absolute `SOUNDGRAMMY_DATA_DIR` override for database, encrypted session file, caches, exports, UI preferences and logs, with a sandbox credential namespace. |
| Daily development still depended on JS wrappers | Introduce a root Cargo workspace and committed root lockfile; remove package.json, Bun lockfile and TypeScript tooling. Replace maintenance scripts with Rust xtask, update CI/release configuration, version checks and editor settings. Desktop emits an ordinary rlib; Android explicitly requests its cdylib. |

The retained service boundaries are unchanged: one native playback owner, account-scoped command ordering, stale-account result rejection, revision-checked queue/playlist edits, and presentation snapshots without Telegram document descriptors. Session encryption and authentication algorithms were not rewritten.

## Verification and visual comparison

- `cargo test --workspace --locked`: **384 application tests + 4 xtask tests passed; one pre-existing ignored application test**. Includes the original migration's desktop/compact production-view tests and added schema, band motion/color, opt-out, recency, nullable sorting and progress regressions.
- `cargo fmt --all --check`, Clippy with `--workspace --all-targets --locked -- -D warnings`, and the release desktop build passed.
- Android arm64 `cargo check --target aarch64-linux-android --no-default-features --features mobile --locked` passed using the installed NDK. An APK/device run was not performed.
- `cargo xtask check-versions` and `cargo xtask icons --check` passed. Windows MSI upgrade identity and Android version-code compatibility have Rust regression coverage.
- Installer configuration generated by the Rust task produced an ad-hoc-signed macOS app. Local packaging used the already available Packager 0.11.2 engine through its NAPI CLI adapter; a globally installed `cargo-packager` executable was not required for this check. The documented user/CI path installs the native Cargo CLI. No installer was published or installed over the user's application.
- Separate original and GPUI audit bundles rendered the same synthetic 5,000-track library, paused track 5 at 1:23/3:16, likes and duplicate-containing custom playlist. Desktop library geometry, player and artwork fallbacks were inspected. Final checks on 2026-11-15 also verified the full-width player with accessibility enabled, the updated fullscreen settings and the generated artwork/palette in fullscreen. Native interaction tests cover search/sort, menus, queue scrolling/edits, selection, dialogs, volume/seek geometry, compact navigation and fullscreen timing/motion. These are stronger interaction checks than screenshots alone, but do not prove live-account behavior.

Windows, Linux, Intel macOS and Android device behavior were not exercised in this audit. Real login/QR/2FA, uncached network playback, export destinations, Last.fm authorization, media keys and hardware/background audio still need an isolated live-account/device walkthrough. Existing tests cover service policy and codec/seek behavior; audible playback was not independently measured here.

## Rust development workflow

From the repository root, configure `native/.env.local` as described in [README.md](../README.md), then use:

```sh
cargo run                         # debug app, separate .dev identity
cargo check --locked              # type/borrow checking without linking
cargo fmt --all                   # format the workspace
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked    # app plus tooling tests
cargo build --release --locked    # optimized executable in target/release
cargo xtask help                  # installer/Android/version/icon tasks
```

Rerun `cargo run` after Rust edits. There is no frontend server/hot reload. Optional `bacon` or `cargo-watch` can rerun checks/builds. `--locked` makes Cargo honor `Cargo.lock`; use `cargo update -p <crate>` only for a deliberate dependency update. Packaging is optional: install `cargo-packager` 0.11.2 once with Cargo, then run `cargo xtask package`. The release workflow's external Release Please service remains a GitHub automation, not a local JS development dependency.

For safe optimized testing:

```sh
SOUNDGRAMMY_DATA_DIR="$(mktemp -d /tmp/soundgrammy-test.XXXXXX)" cargo run --release --locked
cargo run --release --locked -- --preview  # disposable, account-free UI fixture
```

A plain release run uses the production identity. Never launch it for testing without an isolated data root. No production database, session, keychain namespace or app preferences were used in this audit.

## Performance comparison

See [the performance report](performance/report.md) for measured results, run-by-run data, methodology, reproduction commands, the idle CPU investigation and limits. Memory improves substantially in the measured scenario. Follow-up fixes address idle display-link wakeups and demonstrate approximately 120 FPS synthetic native scrolling; the executable remains larger. Audio decoding, Telegram networking and SQLite were already Rust in the original app, so replacing the UI does not imply those operations become faster.


## Follow-up: compact UI and CPU regressions (2026-11-15)

Compact search now matches the original page structure and retains viewport virtualization, visible-row thumbnails, right-click menus and the persistent player. Removed synchronous modal framebuffer readback/CPU blur, retaining a dimmed backdrop; original blur remains a documented visual difference. Embedded ID3 covers cannot be overwritten by late Telegram thumbnails, and both expanded players request high quality. Seek rail layers are square. Cargo-launched macOS development builds set their bundled Dock icon.

The local macOS renderer patch uses the modern window display link at the screen maximum refresh rate and sleeps after idle. Unchanged audio heartbeats no longer redraw every view. See the current [performance report](performance/report.md) for measurements, raw frame/resource data, reproduction commands and remaining limitations. No production session, database, cache, preferences or running application was used.


## GPU and trackpad follow-up

The subsequent [GPU/fullscreen/trackpad follow-up](performance/gpu-follow-up.md) supersedes the blur-removal workaround above. It restores blur using ordered GPU passes, prepares decoded artwork and a palette before fullscreen entry, replaces enlarged baked bands with a display-resolution colour-blend shader, resamples precise pointer scrolling and restores a proportional scrollbar during scroll/drag. Development UI dependencies are optimized through ordinary Cargo profiles; application code stays debuggable. New measurements use actual Metal presentation timestamps and a scroll-position geometry proxy.
