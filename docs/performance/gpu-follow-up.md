# Fullscreen, blur and trackpad follow-up — 2026-11-15

This follows the initial migration/performance audit. The earlier resource table describes the earlier build; it is not a new measurement of this renderer patch. Production databases, session/cache/preferences, credential namespaces and the running production app were not used.

## Blur without the opening freeze

Removing blur was a workaround, not a requirement. The original migrated implementation read a Metal drawable back to CPU memory and resized/blurred it synchronously during modal rendering. That is different from a compositor/GPU blur.

The macOS renderer now recognizes an ordered typed effect surface. While a modal needs blur, it renders into a reusable offscreen texture, downsamples to quarter resolution, encodes horizontal and vertical Gaussian passes, then composites the blur before the sharp dialog/drawer. There is no framebuffer readback, CPU blur, runtime shader compilation or wait for GPU completion in this path. All passes use the same frame command buffer. The extra render targets are released after blur is no longer needed.

Blur still costs GPU bandwidth and offscreen memory; it cannot be literally free. This is bounded work rather than a main-thread screenshot operation. A development probe measured about 22 ms from opening the drawer to the following callback, including initial setup; subsequent presented intervals in that probe were about 8.33 ms. That is a synthetic diagnostic, not an input-to-photon or energy measurement.

## Smooth native fullscreen handoff — 2026-11-16

The app now paints the requested player/library scene before asking AppKit to animate its fullscreen transition. It checks the target native fullscreen state instead of blindly toggling, so opening the player from an already fullscreen library keeps the native window fullscreen. Request generations and account epochs reject superseded callbacks. A regression covers paint-before-transition, the already-fullscreen case and cancelled requests; isolated native inspection verified entry, exit and switching from an already fullscreen library.

The volume popup now owns a focus scope. Focus in any nested control keeps it open when the pointer leaves; focus departure resumes delayed dismissal, which rechecks pointer/focus/drag state before hiding. Regression coverage exercises typing with the pointer outside, blur dismissal and slider drag behavior. Native inspection confirmed exact-value editing stays visible.

The subsequent [repeated benchmark report](benchmark-2026-11-16.html) also identified the one-second display-clock idle grace staying awake between 500 ms caret blinks. The grace is now 50 ms; continuous input and animations still re-arm each frame. The new report separates exploratory pre-fix observations from the final measured binary.

## Ready artwork and palette at fullscreen entry

The app now prepares the current cover while the regular player is visible. A blocking-pool task decodes a bounded cover, extracts its palette and prepares the background source. The fullscreen view consumes an already decoded `RenderImage`, avoiding a second asynchronous image-cache fetch on entry.

If an available fallback is still being prepared, fullscreen entry stays pending in the regular UI until the base cover and palette are ready. A late high-quality/ID3 upgrade keeps the previous ready cover and palette until replacement assets are ready. The previously fixed quality-ordering guard still prevents a remote thumbnail from replacing an embedded cover. Genuine missing/unreadable artwork still uses the fallback icon; the app cannot invent absent artwork.

Tests exercise pending entry, ready base assets, fallback retention during an HQ upgrade and account/task guards. Native inspection used the user-supplied cover screenshot cropped into a disposable fixture. A live Telegram/ID3 transfer was not exercised.

## Palette quality and CSS blend parity

The former approximation baked colour-blended bands into 256 × 160, 8-bit images and enlarged them to the display. That limited spatial/colour precision and blended each band against a precomputed backdrop rather than the actual underlying result.

The new Metal fragment shader evaluates the radial gradients at display resolution, applies group opacity and the W3C nonseparable `color` blend against the current backdrop, then applies the final dark overlay. Palette/band colours remain floats until display output. Spatial dithering reduces quantization bands without adding temporal shimmer. Motion changes small uniforms rather than rebuilding images. Image blur/preparation happens once per source outside the UI thread.

This reproduces the blend mathematics; Rust does not need a CSS engine to express it. It is not a certification of pixel identity across WebKit, GPU drivers, colour spaces, viewport sizes or different cover encodings. The app currently uses this renderer path on macOS; other platforms retain their existing fallback.

## Trackpad scrolling and the visible scrollbar

The user confirmed the development app with a trackpad as the affected scenario, then confirmed high-frame-rate scrolling after the changes.

Three separate effects mattered:

1. A direct 60 Hz stream can yield 120 presents but only approximately 60 distinct scroll positions. The app now resamples precise pointer deltas over at most one input interval, at display cadence. This is short interpolation, not an added momentum/fling engine. OS momentum remains authoritative. Reduced-motion and touch input retain their existing direct behavior.
2. GPUI defaults to an inactive-window animation throttle near 30 FPS. macOS can scroll an unfocused window under the pointer. The app explicitly disables that animation throttle; the native display link still stops at idle. OS thermal protections remain.
3. The previous development profile left GUI/layout/image dependencies unoptimized. The Cargo development profile now optimizes GPUI core/base/components and image kernels at level 2, while application code retains normal development debugging. This causes one dependency rebuild; ordinary `cargo run` remains the workflow.

The shared library/search list now has a proportional scrollbar that appears during scroll activity, stays visible during interaction/dragging and hides after idle. Dragging cancels interpolation so it cannot fight the user's thumb position. A regression paints the visible thumb, drags it to the end and verifies that the final track is visible.

## Dialog input isolation

The Base Kit Dialog handled backdrop presses but its supplied visual backdrop did not occlude GPUI hover hitboxes. The app now occludes both the backdrop and popup surface, including their retained closing animation. Covered track rows, seek previews, volume controls and background scrolling no longer react to the pointer. Left backdrop presses still dismiss the dialog; right presses are consumed without dismissing or opening a background track menu. Opening a modal also clears existing seek/volume hover state and immediately hides the volume popup, whose higher popup priority could otherwise leave it above the dialog during its exit animation.

This uses rectangular input hit-testing on the existing surfaces. It adds no rendering passes, framebuffer readback or image processing and leaves GPU blur unchanged. No new renderer performance measurement was taken for this input-only change. Three new regression tests cover background hover/scroll isolation, closing-animation isolation and immediate removal of an existing volume popup; the existing outside-press test also verifies right-button isolation.

## Presentation measurements

The native fixture probe now records **`MTLDrawable.presentedTime`**, CPU renderer duration and an order-independent hash of glyph vertical positions. This distinguishes actual presentations from repeated unchanged scrolling scenes. The glyph hash is a scroll-position proxy, not a general image/video difference metric; it must not be used to infer FPS of the shader's colour animation.

Raw development results: [presented-frames-development.json](presented-frames-development.json). Earlier release diagnostics are retained in [presented-frames-release.json](presented-frames-release.json); those preceded disabling the inactive-window throttle. The final geometry counter ignores harmless draw-batch reordering. An earlier development diagnostic is retained separately as `presented-frames-development-before-signature-fix.json` and is not used for distinct-position claims.

| Development scenario | Actual presents/s | Distinct glyph-position scenes/s | Opening → next callback |
|---|---:|---:|---:|
| Direct, simulated 60 Hz input | 117.4 | 59.2 | — |
| Resampled, simulated 60 Hz input | 120.0 | 120.0 | — |
| Compact search scroll | 118.0 | 118.0 | 9.33 ms |
| Navigation drawer with GPU blur | 120.0 during the probe | Static after transition | 21.77 ms |
| Fullscreen with prepared cover/palette shader | 119.3 | Not applicable to shader animation | 35.87 ms, including OS fullscreen transition |

These are single diagnostic runs, not repeated population estimates. The first direct-input and drawer runs recorded the window as inactive; the inactive-window cap was disabled. Accessibility was inactive in these automated probes; the separate native UI inspection used accessibility. The fullscreen glyph hash sees control/metadata motion and cannot count background-shader image changes, so its numeric value is intentionally not reported as animation FPS.

Measurements use a 5,000-track synthetic library and one scroll delta every second display callback to approximate 60 Hz input on a 120 Hz display. This bypasses the physical trackpad/OS input pipeline and does not measure input-to-photon latency. The actual native inspection and user confirmation complement this synthetic test.

## Verification and remaining limits

After the dialog input follow-up, 387 native tests plus 4 xtask tests pass; one existing application test is ignored. Formatting, locked workspace checking and all-target workspace Clippy with warnings denied pass. The preceding renderer follow-up also passed locked release/development builds and Android arm64 compilation. Native UI inspection of that renderer follow-up confirmed the restored blur, a coloured fullscreen background with ready artwork, and the scrollbar appearing during scroll. Regression tests cover scrollbar dragging and intermediate scroll positions with no added post-input momentum.

GPU blending/blur is implemented through the local `gpui-pre-apple` patch, documented in [SOUNDGRAMMY-PATCH.md](../../native/vendor/gpui-pre-apple/SOUNDGRAMMY-PATCH.md). The original macOS frame-source patch remains separate. Intel/discrete GPU and old macOS runtime behavior, Android devices, live audio/streaming, animated energy usage and long-session memory have not been measured.
