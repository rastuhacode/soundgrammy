# Dependency maintenance

Daily development and CI use Rust 1.98.1, selected by `rust-toolchain.toml`.
The application uses GPUI Kit 0.10.0, which pins GPUI 0.3.12. The Android host
and Apple renderer use that same GPUI release. GPUI 0.3.13 requires a compatible
Kit release. The resolved graph also keeps wgpu 30.0.1 for GPUI Fast's exact
requirement. Upgrade these packages as a compatible set.

Run `cargo xtask check-dependencies` after changing the graph. CI runs it too.
It verifies the reviewed Kit/renderer pair, the registry source for the Apple
renderer, and the pinned Android host patch. Update this policy deliberately
when reviewing an upgrade or removing a patch.

## Local patches

| Package | Purpose | Removal condition |
|---|---|---|
| `gpui-pre-mobile` 0.1.0 | Align the Android host at upstream revision `7548ebc4` with GPUI/wgpu 0.3.12 | A compatible upstream host release or revision is available. |

The Android patch includes `SOUNDGRAMMY-PATCH.md` with provenance and
implementation details. Preserve its upstream license notices.

The Apple renderer now comes from crates.io. The drawer and modal dialogs use
an animated dim backdrop, and fullscreen uses artwork layers prepared off the
UI thread on every platform. [GPUI Kit #3598](https://github.com/longbridge/gpui-kit/issues/3598)
tracks upstream in-window backdrop blur.

The old Metal renderer (`gpui-pre-apple` 0.3.10 → `metal` 0.33 →
`block` 0.1.6) triggered Rust's warning about an uninhabited foreign static.
GPUI's Apple renderer switched to `objc2-metal` and `block2` in 0.3.11.
With 0.3.12 the local `block` override is gone. Cargo still records registry
`block` 0.1.6 through the disabled `screen-capture` feature
(`zed-scap` → `cocoa` / `cocoa-foundation`). The application build does not
compile that crate. Verify with `cargo tree --locked --all-features --target all -i block`.

The workspace now uses registry `ferogram` and `ferogram-mtsender` 0.6.7.
Upstream releases a slot lease on cancellation and publishes additional
connections after initialization. This supersedes our 0.6.5 sender override.
The macOS window backend also comes from the registry. It paces frames with
`CAMetalDisplayLink` and pauses while idle. Its refresh range follows macOS
display policy.

The app's Android bridge uses JNI 0.22.4. The Android credential store still
depends on JNI 0.21 internally. The mobile host uses `libc` 0.2.191, which
restores the iOS symbols that backtrace needs.

## Upgrade procedure

1. Review the new Kit's renderer/platform dependencies and mobile host revision.
   Compare each local patch with the matching new upstream source. Port a still
   required patch or remove its Cargo override, source tree and policy entry.
2. Update the Cargo lockfile deliberately, then run `cargo xtask check-dependencies`,
   `cargo fmt --all --check`, Clippy, and `cargo test --workspace --locked`.
3. Use the account-free `--preview` fixture to exercise scrolling, menus, settings,
   seek/volume controls, fullscreen entry/exit, reduced motion and accessibility.
4. For macOS renderer changes, also check high-refresh and external displays,
   inactive/minimized windows, drawer layering, wake from idle and repeated close.
   The account-free frame probe reports callback cadence, not actual presents.
5. Run desktop CI on macOS, Windows and Linux and build Android arm64. Mobile
   changes additionally require an Android device check of input, Activity
   recreation, background playback and OS media controls.

A successful cross-check or headless interaction test does not establish device
or display behavior. Record which targets were actually exercised in the change
report. Release identity, storage paths and signing compatibility are separate
acceptance requirements and must survive dependency updates.
