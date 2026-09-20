# Native audio

## Run

```sh
bun tauri:dev
```

Rust is the only desktop playback engine in development and release builds.
No environment variable is required.
The native service and its output dependencies are enabled on desktop, Android,
and iOS. Android requires API 26 or newer for CPAL’s AAudio backend. Initialization
is lazy; a missing device does not prevent startup. Mobile foreground playback
still requires device verification; enabling compilation does not establish parity.

Rust owns queue progression, repeat/shuffle, playback intent, listening statistics,
and Last.fm attempt qualification. React mirrors native snapshots and handles in-app
keyboard shortcuts. OS metadata and remote actions use the [native media bridge](native-media-bridge.md),
without browser Media Session handlers.

Android has a foreground playback service, audio-focus handling, and a CPU wake lock.
iOS has playback audio-session/interruption integration and background-audio configuration.
These implementations are not yet verified on physical devices; iOS compilation is
also outstanding. See [mobile background playback](mobile-background-playback.md)
for current build evidence and acceptance gates, and [audio engine boundary](audio-engine.md)
for ownership and UI reattachment. Queue/position survive WebView recreation in the
same process, but are not persisted across process termination.

## Implementation

- `audio/android.rs`: initializes CPAL’s NDK context from a retained Android Application
  before the first audio command; no Activity is retained.
- `audio/mod.rs`: bounded 32-command control queue and output ownership on one
  dedicated thread; per-generation decode workers and bounded message channels.
- `audio/activity.rs`: PCM-based listen attempts, local statistics, and ordered Last.fm qualification.
- `audio/media/`: native metadata and command adapters for desktop and mobile.
- `audio/lifecycle.rs`, `audio/ios.rs`, Android `PlaybackService.kt`: interruption policy and mobile playback lifetime.
- `audio/session.rs`: native queue and mode policy, attempt identity, serial user
  commands and natural-end transitions, and revisioned UI snapshots.
- `audio/shuffle.rs`: native random, variety, rediscover, smart, fresh, and duration
  permutations, with current-membership pinning and base-order restoration.
- `audio/source.rs`: cached files or an independent seek cursor with a 128 KiB
  verified range cache. Missing blocks use the existing `TrackStream` downloader;
  blocking decoders wait on bounded replies, checking cancellation every 25 ms.
  Sparse holes are never treated as downloaded bytes. There is no audio-byte IPC.
- `audio/decode.rs`: Symphonia byte probing, packet decoding, coarse demuxer
  seeks and preroll discard, conversion and bounded producer backpressure.
- `audio/convert.rs`: mono/stereo mapping and persistent Rubato resampling.
  Multichannel input currently returns unsupported-format; stereo uses the first
  two output channels and additional output channels receive silence.
- `audio/output.rs`: a two-second SPSC PCM ring, 50 ms startup watermark,
  atomic gain, silence on underrun, and a bounded presentation timestamp ledger.
  The callback performs no allocation, mutex locking, decoding, I/O, logging,
  network requests, or event emission. CPAL timestamps account for device delay;
  natural completion waits for the final queued frames to be presented.
- `native-engine.ts`: command serialization, payload validation, identity and
  revision filtering, startup/resume snapshot reconciliation, and scrubbing.
- `engine-factory.tsx`: creates the native adapter for each provider lifetime.
  Format and output errors are surfaced without switching playback engines.

State is published on `audio:state`; natural-end edges use `audio:event`.
Both include revision and track/attempt identity. Steady playback updates are
limited to about 7 Hz. Buffered ranges describe locally available audio. Cached
files show the full track. For partial files, a storage-only demux observer maps downloaded packets
using their actual timestamps; it neither decodes PCM nor requests downloads.
Coverage persists behind playback and across seeks. Unmapped data stays unpainted
until its container metadata/packets can be read; no byte/time ratio is guessed.
A seek reuses the output stream, source, demuxer/index, and decoder worker. A
latest-wins mailbox cancels obsolete range waits. The producer stops and the
callback acknowledges a bounded PCM/presentation-clock flush before new samples
are produced. The decoder and resampler reset for the new position. MP4 playback
stops at its declared sample-table end to preserve Symphonia's seekable atom state.
The cursor moves immediately, with a seeking indicator until audio is ready;
listening activity resumes only on actual presentation. Pausing while waiting for
source data cancels that pipeline; Play retries from the displayed position. Scrubbing keeps only its final requested seek.
Debug tracing records demux seek duration and total request-to-ready time
(`RUST_LOG=soundgrammy_lib::audio=debug`).

The downloader is allowed to settle an already in-flight chunk after cancellation
so shared transfer manifests cannot be stranded in Loading. Its active token
prevents scheduling further playback requests. Existing export/cache download
owners retain their independent lifetime.

## Dependencies and platform requirements

| Dependency | Purpose | License / compatibility |
| --- | --- | --- |
| CPAL 0.18.2 | Desktop/mobile host/device selection and sample output | Apache-2.0; default optional hosts disabled |
| ringbuf 0.5.2 | Bounded lock-free SPSC PCM transfer | MIT/Apache-2.0; no platform audio dependencies |
| Rubato 0.16.2 | Stateful, band-limited FFT sample-rate conversion | MIT; portable Rust; mature API compatible with the current Rust 1.97 toolchain |

Rubato was chosen to avoid an ad-hoc resampler and preserve continuity across
packet boundaries. Input is chunked persistently, startup delay is removed once,
and the final tail is flushed and trimmed to the exact output frame count.
The 0.16 API line is pinned for this implementation; newer major API migrations
should retain the continuity and frame-accounting tests.

Upstream documentation: [CPAL](https://docs.rs/cpal/0.18.2/cpal/),
[ringbuf](https://docs.rs/ringbuf/0.5.2/ringbuf/),
[Rubato](https://docs.rs/rubato/0.16.2/rubato/).
Linux needs ALSA development headers (`libasound2-dev` on Ubuntu), in addition to
the existing Tauri packages. CI now checks macOS, Windows, and Linux headlessly.

## Verification record

Current host recheck (2026-09-20): 260 frontend tests and 159 Rust library tests
passed. These tests do not establish mobile device behavior. The dated entries below
record earlier migration stages; current mobile status is maintained in
[mobile background playback](mobile-background-playback.md).

Native session migration automated validation (2026-09-16): 258 frontend tests
and 142 Rust tests passed, including 3,000 native state transitions, all shuffle
modes, duplicate memberships, stale commands/seeks, and UI reattachment races.
Review regressions cover unpinned playlist shuffle and delayed completion events
after the native session has advanced, including duplicate-track attempts.
Frontend lint, TypeScript/production build, Clippy with warnings denied, and Rust
formatting passed. This is headless validation; audible hardware playback and
mobile background execution were not exercised by this migration.

Local macOS automated validation (2026-09-15): frontend tests, Rust tests,
ESLint, Clippy with warnings denied, TypeScript, Rust formatting/check, and the
production frontend build passed. Cross-platform CI results are pending.

Synthetic fixture tests cover actual codec/container combinations; they do not
establish parity for all files carrying those extensions.

| Fixture | Decode / finite PCM / middle seek / EOF |
| --- | --- |
| MP3 with ID3v2 and VBR metadata | Pass |
| M4A AAC | Pass |
| M4A ALAC | Pass |
| Ogg Vorbis | Pass |
| FLAC | Pass |
| WAV PCM | Pass |
| Ogg Opus | Explicit unsupported-format (playback error) |
| WebM Opus | Explicit unsupported-format (playback error) |

Headless tests additionally cover resampling both 44.1→48 and 48→44.1 kHz,
packet-boundary continuity, gain/sanitization, silent underrun, presentation
latency/tail accounting, range-cache reuse, seek prioritization, start before
complete download, cancellation during a missing-range wait, corrupt input,
proxy contract, stale events, command failure, and native teardown ownership.
Fixtures and provenance are in `src-tauri/tests/fixtures/audio/`.

Run validation with `bun lint`, `bun typecheck`, and **`bun run test`** (the
package script runs Vitest and Cargo). Bare `bun test` invokes Bun's own test
runner and does not provide the Vitest/jsdom test environment. Also run
`cargo fmt --manifest-path src-tauri/Cargo.toml --check` and `cargo check
--manifest-path src-tauri/Cargo.toml`.

### Release gates still requiring device verification

The following have **not** been verified by
this change's headless tests:

- Real Telegram-library files and throttled network playback across the format
  matrix, including M4A indexes at the end of the file.
- Debug/release hardware playback on macOS, Windows, and Linux; no-device paths,
  44.1/48 kHz devices, Bluetooth removal, sleep/wake, and shutdown.
- Queue/repeat/duplicate rows, listening statistics, and Last.fm during real
  output (native policy and accounting are covered by unit tests).
- A one-hour mixed-format run measuring memory, CPU, underruns, and worker count.
- Fuzzing and adversarial container allocation behavior inside demuxers. This
  implementation caps metadata, packet processing, decoded buffers, and prefetch;
  it does not claim an allocation sandbox around third-party decoders.

Record device verification results separately from automated checks.

### Playback regression fixes

Play acknowledges a buffering state before output resumes, so the frontend does
not interpret the command reply as another Pause. Main WebView navigation stopped
native playback in the original transport-only implementation. The native-session
migration now keeps playback running and only invalidates queued old-page commands;
logout/revocation explicitly clears the session.

Canceled reads return ConnectionAborted, not Interrupted (which read_exact
retries indefinitely). This prevents canceled decoder/coverage workers spinning.
Seeks use the demuxer coarse mode, avoiding MP3's sequential accurate-mode scan.
MP3 landing time can be approximate, especially for variable bitrate files;
container-indexed formats use their demuxer's seek implementation. A two-minute
MP3 regression checks playable output at 75% with the middle chunks untouched.
Real-device CPU and network playback still require a follow-up run.

### Mobile target enablement (2026-09-17)

Removed desktop-only audio gates, including command registration, streaming
observation, active-cache protection, and logout cleanup. Shared dependency
versions are unchanged. Android minimum SDK is 26 in both Tauri configuration
and the generated Gradle project. CPAL’s Android context is initialized lazily
using a process-lifetime Application global reference before any output access.

Validation: desktop `cargo check`, Android ARM64 `cargo check` using NDK 29/API 26,
Rust formatting, and 45 desktop audio tests passed. No phone playback was tested.
iOS compilation was not checked: this environment lacks the iOS Rust target and
full Xcode/iOS SDK. At that milestone, native media controls and background lifecycle were not yet
implemented. Subsequent changes added both, including the Android foreground
service; see [mobile background playback](mobile-background-playback.md) for current status.
