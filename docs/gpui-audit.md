# GPUI migration audit — 2026-11-14

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
