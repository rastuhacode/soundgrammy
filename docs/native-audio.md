# Native audio

Rust owns playback on desktop and mobile. React sends commands, renders native snapshots, and handles in-app keyboard shortcuts. See the [README](../README.md#local-development) for app setup.

## Queue and UI boundary

- [`audio/session.rs`](../src-tauri/src/audio/session.rs) owns queue order, cursor, source, play intent, repeat, shuffle, and attempt identity. Duplicate track IDs are distinct queue entries. One control worker serializes commands and queue progression. Listen accounting uses presented PCM through [`audio/activity.rs`](../src-tauri/src/audio/activity.rs).
- The [engine provider](../src/hooks/audio/engine-factory.tsx) owns the IPC adapter and subscriptions. The [native adapter](../src/hooks/audio/native-engine.ts) and player command port share an account-scoped command queue. Tests can inject both ports. Zustand stores apply acknowledged state, and stale position-based edits are rejected using the viewed session revision. Command failures surface in the UI and trigger a snapshot refresh.
- Transport and session snapshots have independent revisions. A position tick can arrive before a resume snapshot that carries a newer queue. The adapter subscribes before attaching, validates payloads, and reconciles on `pageshow` and return to visibility. Delayed seeks stay scoped to the original track attempt.
- WebView navigation invalidates commands queued by the old page. Detaching the UI leaves playback and its current attempt running. Explicit clear, logout, and revoked sessions clear the player and OS metadata. After process exit, a fresh player starts idle. Repeat and shuffle preferences persist in SQLite, and saving the queue as a playlist preserves track order.

## Decode and output

| Stage | Contract |
| --- | --- |
| [Source](../src-tauri/src/audio/source.rs) | Opens complete cached audio directly or blocks on verified ranges. Range waits observe cancellation. See [streaming](streaming.md) for request ownership and the buffer display. |
| [Decode](../src-tauri/src/audio/decode.rs) | Symphonia probes and decodes the container. Seeks use the demuxer's seek support with preroll discard. Decoder and resampler state reset after a seek. |
| [Conversion](../src-tauri/src/audio/convert.rs) | Mono and stereo streams are mapped to the device and resampled with persistent Rubato state. Preserve continuity across packets and exact frame accounting through the final flush. |
| [Output](../src-tauri/src/audio/output.rs) | CPAL tries the current device format first, then supported alternatives. A bounded single-producer, single-consumer PCM ring feeds the callback. Keep allocation, locks, I/O, and event emission outside that callback. Device timing determines when the last queued samples have been presented. |

The native worker reports state on `audio:state` and natural completion on `audio:event`. Rust advances the queue. The transport cursor reflects presentation, while the buffer display reflects verified packet ranges independently of playback position.

## Verification

The [codec fixtures](../src-tauri/tests/fixtures/audio/README.md) and [decoder tests](../src-tauri/src/audio/decode.rs) cover decoding, finite PCM, seek, and EOF. The Ogg Opus and WebM Opus fixtures exercise the playback error path. Linux builds require ALSA development headers in addition to the normal Tauri dependencies. The [CI workflow](../.github/workflows/ci.yml) shows the current build matrix.

Run `bun run test` for Vitest and Cargo. Use the [platform playback guide](platform-playback.md#device-verification) for checks that need actual audio output or an OS media surface.
