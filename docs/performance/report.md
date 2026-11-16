# Tauri → GPUI performance and regression report

Updated 2026-11-15. Original UI: `develop` **745ef82**. Native UI: this worktree, Rust + GPUI Kit 0.8.1 / GPUI 0.3.10, with the local macOS frame-pacing patch.

## Latest follow-up

The [2026-11-16 HTML report](benchmark-2026-11-16.html) contains a new repeated workload matrix after the fullscreen-transition and volume-focus fixes. See its [method and reproduction guide](benchmark-method-2026-11-16.md). It supersedes the older resource numbers below for the current build.

The new 78-launch release matrix measures 328.68 → 140.86 MiB median RSS for a paused 5,000-track library, 55.12 → 27.31% of one core during desktop scrolling, and 31.78 → 16.41% during synthetic fullscreen playback animation. The populated startup marker is 464.88 → 245.27 ms. The executable remains larger (34.56 → 52.12 MiB). Focused filtered search has a small residual CPU difference (0.60 → 0.72%). The HTML includes all states, individual launches, different frame instruments, eight development launches and two extended artwork-scroll runs; these are synthetic UI workloads, not real network/audio-output measurements.

See [the GPU/fullscreen/trackpad follow-up](gpu-follow-up.md) for restored GPU blur, prewarmed fullscreen artwork, full-resolution colour blending, development-build scrolling and visible/draggable scrollbars. The resource table below was measured **before** that GPU follow-up; the older blur-removal/renderer descriptions below are historical findings, superseded where indicated in the follow-up.

## Resource results before the GPU follow-up

| Metric | Original Tauri | Migrated GPUI | Comparison |
|---|---:|---:|---|
| Aggregate idle RSS | 327.05 MiB | 136.59 MiB | 58.2% lower |
| Idle CPU, one core = 100% | 0.05% | 0.05% | Approximately equal; near sampling floor |
| App + helper processes | 4 | 1 | Three fewer |
| Release executable | 34.56 MiB | 52.00 MiB | 50.4% larger |

[Final raw resource measurements](benchmark-after-frame-pacing.json). GPUI idle CPU fell about **93.8%** from the prior 0.81% median to approximately 0.05% of one core. Small readings are noisy; this does not mean zero work or that GPUI is measurably faster than Tauri at idle.

| Run | Tauri RSS | GPUI RSS | Tauri CPU | GPUI CPU |
|---|---:|---:|---:|---:|
| 1 | 328.64 MiB | 139.97 MiB | 0.10% | 0.05% |
| 2 | 326.61 MiB | 135.16 MiB | 0.00% | 0.00% |
| 3 | 327.05 MiB | 136.59 MiB | 0.05% | 0.25% |

Resource measurements use release builds, a 1,200 × 800 logical window, 5,000 synthetic tracks, paused track 5 at 1:23, no audio output/network activity and no artwork. Three alternating runs per app, ten seconds warmup, twenty one-second samples. RSS includes the Tauri/WebKit helpers. Helpers are attributed by process-tree membership plus WebKit processes appearing during that audit launch; unrelated concurrent WebKit launches would confound this heuristic. Final runs consistently recorded one app and three helpers. The scripts terminate only their launched process groups and leave XPC teardown to macOS. One CPU core equals 100%.

Before these fixes, the same fixture measured **0.81% GPUI idle CPU versus 0.10% Tauri** (medians). That was a small absolute increase of 0.71 percentage points, despite the large relative multiplier. See [the earlier repeated measurements](macos-5000-tracks.json). The first follow-up run overlapped compilation and had an anomalous 7.3% Tauri observation; [its raw data](benchmark-after-frame-pacing-build-load.json) is retained separately and excluded from the final table.

## Scrolling and opening overlays

[Raw frame intervals](frame-pacing.json), three runs per scenario:

| Scenario | Median cadence | Median p95 interval | First opening → next frame |
|---|---:|---:|---:|
| Original desktop track scrolling | 60.0 FPS | 17.00 ms | Not measured |
| GPUI desktop track scrolling | 120.0 FPS | 8.36 ms | — |
| GPUI compact search scrolling | 120.0 FPS | 8.36 ms | 8.29 ms median; 8.31 ms maximum |
| GPUI account/navigation drawer | Callback probe, see caveat | See caveat | 8.77 ms median; 10.51 ms maximum |

The native probe schedules one synthetic 18-pixel scroll input per display callback; the original schedules one 18-pixel DOM scroll step per `requestAnimationFrame`. Each excludes the first thirty callbacks and records 360 intervals over a populated, virtualized list. Native compact scenarios use a 390 × 844 window. These measure application callback cadence under scrolling and the first callback following an opening render. They **do not measure input-to-photon latency, GPU presentation timestamps or a user's physical trackpad**. The original WebView's 60 FPS result describes this probe on this machine, not a universal upper bound on WebKit scrolling, which can also use compositor paths.

Two drawer runs stayed near 120 callbacks/second; the third averaged 98.6 with p95 33.34 ms. GPUI retains its upstream inactive-window (~30 FPS) and thermal protections. The drawer scenario generates no high-rate input, so this result cannot establish uninterrupted 120 FPS animation across focus/OS state changes. The scrolling scenarios did sustain approximately 120 FPS. The decorative equalizer's 30 Hz timer is confined to its own view; it does not cap list scrolling.

## What caused the regressions and what changed

### Drawer/search freezes

The migration called `window.render_to_image()` inside modal rendering, performed a synchronous Metal readback, resized the whole image and blurred it on the UI thread. Search and the navigation drawer both passed through this path. A virtualized list did not prevent that stall.

The framebuffer capture/CPU blur module and its production `test-support` dependency were removed. Dialogs retain their dimmed backdrop. Compact search now follows the original non-modal page layout: back arrow and search input, result count and playlist name, viewport-sized virtual list, and the persistent player below it. The background track table is not mounted a second time while search is open. Visible search rows request thumbnails. Compact rows now support right-click menus as well as the options button.

**Remaining visual difference:** navigation/settings backgrounds are dimmed without the original backdrop blur. Recreating that blur needs a renderer/compositor implementation; restoring synchronous framebuffer capture would restore the stall.

### Idle CPU and display cadence

The previous [native stack sample](gpui-idle.sample.txt) identified the visible-window `CVDisplayLink` thread as the conspicuous recurring idle work. The published macOS backend ticked continuously even when GPUI had nothing to draw.

The repository now vendors only `gpui-pre-macos`, with [a documented patch](../../native/vendor/gpui-pre-macos/SOUNDGRAMMY-PATCH.md):

- Prefer macOS 14+ `NSView`/`CADisplayLink`, configured for the screen's maximum refresh rate and common run-loop modes, including window tracking. Keep the original CoreVideo implementation on older systems.
- Connect GPUI's `frame_waker` and `schedule_frame` to the native clock. Stop the link after one second without frame demand; resume on input, notifications or animation requests.
- Invalidate the modern link on idle/occlusion/resize/close to release its retained view. Preserve the upstream legacy link registry's teardown protections.

The UI also accepts unchanged audio heartbeat revisions without notifying every view. Changed position, buffer coverage, loading/seeking state, volume, errors and full session updates still update the UI. A regression verifies both the absence of redundant row rendering and delivery of changed buffer state.

A normal `cargo run` is an unoptimized development build. Compare release builds against release builds when assessing the migration; this report does not equate development CPU readings with these release measurements. Active playback, high-refresh drawing and continuously moving fullscreen layers necessarily perform work; the synthetic idle result is not a claim of zero CPU in those modes.

### Artwork upgrades

Fullscreen already requested the backend's bounded leading-ID3 probe, but a slower Telegram full-thumbnail response could overwrite an embedded cover delivered by `artwork:changed`. Embedded artwork now wins regardless of completion order. High-quality requests are also made for the expanded compact player, and reopening either expanded view retries a previous deferred request. High-quality work is scheduled before the normal fallback request.

The existing service still owns the bounded ID3 probe, chunk coordination, timeout, account guard and cache files. This change does not move Telegram descriptors into the UI or rewrite authentication/session crypto. Tests cover response ordering; an account-free fixture verifies display, not a real Telegram download.

### Other requested fixes

All seek rail, buffer, played-fill and loading-stripe layers have square edges; the interactive thumb remains circular. macOS startup installs the bundled application image on `NSApplication`, so launching the development executable with Cargo does not rely on a packaged app for its Dock icon. The required keys are present in this worktree's ignored `native/.env.local`; their values were not printed or copied from production storage.

## What improved, what costs more, and limits

The migration removes WebView/helper processes and frontend JSON/JavaScript state mirrors. Track lists reuse cached projections and mount only viewport rows; decoded cover caching is bounded at 96 items. Audio decoding, MTProto, streaming, SQLite and most service logic were already Rust, so the UI migration alone does not establish faster codecs or transfers.

The release executable remains larger. Tauri uses the OS-provided WebKit runtime outside its executable; GPUI carries its renderer/toolkit/assets. Installer size is different from executable size and was not benchmarked. No asymmetric release LTO/strip settings were introduced to improve one side of the comparison.

Measured machine: macOS 27.0.1 (26A434), Mac16,8, Apple Silicon, 24 GiB memory, 12 logical CPUs. Measurements can vary with focus, OS memory reclamation, display configuration, thermal conditions and other desktop activity. Both apps were inspected with populated fixtures; blank/login surfaces are invalid baselines.

**Not measured:** authenticated account hydration, uncached streaming/download throughput, decoder/output CPU while playing, buffer underruns, seek latency, live ID3 completion time, artwork-heavy browsing, fullscreen animation CPU/GPU/energy, startup-to-first-painted-frame, long-session memory, Android device FPS, Windows/Linux/Intel Mac performance, or installer/update size. The macOS 14+ path was exercised on this machine; the legacy fallback was compiled but not run on an older OS. Production data and the running production app were not used.

## Verification

384 application tests and 4 xtask tests pass; one pre-existing application test remains ignored. Formatting, workspace/all-target Clippy with warnings denied, locked release desktop build and Android arm64 mobile cross-check pass. Native UI inspection confirms compact search/filter/clear, right-click menus, dismissal, drawer wake after idle, expanded fixture artwork and square seek rails. Live Telegram authentication/playback was not exercised.

## Reproduce without production state

Use the commands in [the audit guide](../gpui-audit.md) to prepare the isolated original at `develop`, applying [the audit-only adapter](tauri-fixture.patch). That patch assigns an audit identity and temporary data/cache paths, avoids loading a real session, and supplies fixture API responses. Its optional frame probe changes only the audit adapter, not the original layout components. Build with the `tauri/custom-protocol` feature so the baseline loads bundled frontend assets.

Build the native release binary with `cargo build --release --locked`; copy both binaries into temporary `.app` bundles with unique audit identifiers. Do not replace an installed application. The samplers reject non-temporary executable paths and pass isolated state/fixture options:

```sh
python3 tools/make_benchmark_fixture.py /private/tmp/soundgrammy-audit-fixture.json
python3 tools/benchmark.py \
  --tauri '/private/tmp/soundgrammy-audit-apps/Tauri Audit.app/Contents/MacOS/tauri' \
  --gpui '/private/tmp/soundgrammy-audit-apps/GPUI Audit.app/Contents/MacOS/gpui' \
  --fixture /private/tmp/soundgrammy-audit-fixture.json \
  --output /private/tmp/soundgrammy-resources.json
python3 tools/benchmark_frames.py \
  --tauri '/private/tmp/soundgrammy-audit-apps/Tauri Audit.app/Contents/MacOS/tauri' \
  --gpui '/private/tmp/soundgrammy-audit-apps/GPUI Audit.app/Contents/MacOS/gpui' \
  --fixture /private/tmp/soundgrammy-audit-fixture.json \
  --output /private/tmp/soundgrammy-frames.json
```

The native frame probe is inactive during normal launches. It requires `--preview-fixture`, `SOUNDGRAMMY_PREVIEW_PERF` pointing beneath `/private/tmp`, and an optional `SOUNDGRAMMY_PREVIEW_PERF_MODE` (`scroll`, `search`, `drawer`). For interactive compact inspection, use `--preview-fixture … --preview-compact` without the performance environment variables.
