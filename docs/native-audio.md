# Native desktop audio

## Run

```sh
bun tauri:dev
```

Rust is the only desktop playback engine in development and release builds.
No environment variable is required.
The native service and its output dependencies compile only on macOS, Windows,
and Linux. Initialization is lazy; a missing device does not prevent startup.

This implements desktop playback. Android/iOS background playback still needs
platform audio lifecycle integration and a queue owner that survives WebView
suspension. Queue, repeat/shuffle, listening statistics, Last.fm, shortcuts, and
browser Media Session controls continue through the existing adapter.

## Implementation

- `audio/mod.rs`: bounded 32-command control queue and output ownership on one
  dedicated thread; per-generation decode workers and bounded message channels.
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
  revision filtering, startup subscription/snapshot reconciliation, and scrubbing.
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
| CPAL 0.18.2 | Desktop host/device selection and sample output | Apache-2.0; desktop target dependency; default optional hosts disabled |
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
  output (shared frontend behavior is covered by unit tests).
- A one-hour mixed-format run measuring memory, CPU, underruns, and worker count.
- Fuzzing and adversarial container allocation behavior inside demuxers. This
  implementation caps metadata, packet processing, decoded buffers, and prefetch;
  it does not claim an allocation sandbox around third-party decoders.

Record device verification results separately from automated checks.

### Playback regression fixes

Play acknowledges a buffering state before output resumes, so the frontend does
not interpret the command reply as another Pause. Main WebView navigation stops
native playback and invalidates commands from the previous page.

Canceled reads return ConnectionAborted, not Interrupted (which read_exact
retries indefinitely). This prevents canceled decoder/coverage workers spinning.
Seeks use the demuxer coarse mode, avoiding MP3's sequential accurate-mode scan.
MP3 landing time can be approximate, especially for variable bitrate files;
container-indexed formats use their demuxer's seek implementation. A two-minute
MP3 regression checks playable output at 75% with the middle chunks untouched.
Real-device CPU and network playback still require a follow-up run.
