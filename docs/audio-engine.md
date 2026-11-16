# Audio engine

The Rust audio service owns the queue, repeat/shuffle preferences, current attempt, desired playback and listening accounting. GPUI sends ordered intent and adopts revisioned display snapshots. UI remounting, Android Activity recreation and view subscriptions never create a second decoder or listening attempt.

Player commands resolve track metadata from account-scoped SQLite rows. Queue/source membership retains duplicate positions; positional edits require the expected order or queue revision. All six shuffle modes, repeat boundaries, previous-track restart, manual queue edits and source reconciliation remain in `native/src/audio/session.rs` and `audio/shuffle.rs`.

Streaming uses verified downloaded byte ranges, Symphonia decoding, PCM conversion/resampling, a bounded ring and CPAL output. Seeks are bounded, attempt-aware and cancellation-safe. Buffering recovery waits for useful media headroom rather than expanding PCM memory. Listen duration follows consumed PCM, so seeking and buffering do not inflate statistics or Last.fm qualification.

Desktop teardown stops audio before dropping media controls and resources. Android keeps the core process-owned through view replacement and background playback. Explicit clear/unload, logout/revocation, task removal and process exit retain their existing stop semantics. Queues survive view recreation but are not persisted across process death.

The native service tests cover queue, shuffle, duplicate attempts, completion ordering, stream cancellation, codec/seek behavior and revision races. GPUI tests cover command intent, focus, presentation and reattachment guards. See [migration evidence](gpui-migration.md) for current builds and remaining hardware acceptance.
