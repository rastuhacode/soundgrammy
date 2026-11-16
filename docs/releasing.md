# Releasing

Release Please still derives versions and changelogs from Conventional Commits on `main`, creates a tag and a draft GitHub Release. The native build workflow uploads macOS DMGs, Windows EXE/MSI installers and the Android arm64 APK to that draft.

## Versioning

`version.txt`, `native-app.json`, `native/Cargo.toml`, `Cargo.lock` and `.release-please-manifest.json` carry the release version. `native-app.dev.json` derives `<version>-develop`. Run:

```sh
cargo xtask check-versions
cargo xtask check-versions --fix
```

The Release Please workflow synchronizes the derived development version on its release PR. Android versionCode preserves the existing scheme: `major * 1_000_000 + minor * 10_000 + patch * 100`.

## Building locally

```sh
cargo install cargo-packager --version 0.11.2 --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask package
cargo xtask package --target x86_64-apple-darwin
cargo xtask android
```

`cargo-packager` packages the Rust binary without a WebView or frontend distribution. `cargo xtask package` builds with the committed Cargo lockfile and generates an ignored packaging configuration from the same app identity used at runtime. Output is under `dist/<target-or-platform>/<profile>/`. `--debug --formats app` creates an isolated macOS development bundle; `--no-build` packages an existing binary.

Official release builds require Telegram and Last.fm application credentials in CI secrets. Android additionally requires `ANDROID_KEY_BASE64`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`; the workflow decodes the keystore to a runner temporary file and supplies `ANDROID_KEYSTORE_PATH` to Gradle. Keep the existing signing key to allow upgrades of installed apps. Windows MSI packaging uses the existing Tauri 2 UpgradeCode rather than cargo-packager's different default; the retained template and provenance are in `native/packaging/`. The derivation follows [Tauri's MSI implementation](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/msi/mod.rs). macOS builds use ad-hoc signing unless distribution signing/notarization credentials are configured.

## Workflow and acceptance

CI tests Rust services, GPUI interactions, Rust xtask tooling and desktop builds on macOS, Windows and Linux. Android CI builds the arm64 native library and APK and runs JVM tests. Release builds package Apple Silicon and Intel macOS plus Windows and signed Android.

The manually dispatched release workflow accepts an existing tag or exact SHA. The optional lock repair updates the application package version before version validation. It does not replace testing of the intended release commit.

Before publishing the draft, run platform acceptance: sign-in/2FA/QR, streaming/cached playback and seeks, duplicate playlists/queue edits, downloads/recipe dialogs, OS media keys, fullscreen behavior, logout and update from a previous release. Android additionally needs IME composition, notification controls, background/lock-screen playback, audio interruptions, Back, Activity recreation and task removal. Build success is not a substitute for those device checks. The GPUI migration's physical Android checks are deferred to the user.
