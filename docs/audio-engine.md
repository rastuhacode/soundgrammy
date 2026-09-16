# Audio engine boundary

Desktop playback uses `NativeRustAudioEngine`, created by `engine-factory.tsx`. The provider owns an IPC adapter and subscriptions, not the native playback lifetime. Tests can inject a fake engine.

`audio/session.rs` is the authoritative queue state machine. It owns current membership, queue edits, repeat/shuffle, desired playback, and monotonic attempt identifiers. `audio/shuffle.rs` implements the six shuffle modes using native library statistics. The transport control thread processes user commands and natural completion serially; advancing or repeating a track does not need JavaScript. Duplicate track IDs remain distinct memberships, and reshuffling/reordering preserves the current attempt.

The player, repeat, and shuffle Zustand stores mirror acknowledged native state. UI actions send `native_player_command` through `api.ts`; they do not optimistically replace playback state. The UI supplies playlist membership in its displayed sort order, while Rust applies shuffle and chooses the current row. Index-based edits include the viewed session revision. Rust rejects stale edits rather than removing, moving, or jumping to a different row. Command failures are visible in the player and trigger a snapshot refresh.

The native adapter subscribes before attaching, validates payloads, and rejects stale transport revisions. Full snapshots include the session and its independent revision; position-only events omit the potentially large queue. A full resume snapshot can hydrate a missed queue change even if a newer position tick arrived first. The adapter accepts native-generated track/attempt identities and reconciles again on `pageshow` and visible `visibilitychange`. Seeking is scoped to the original attempt, so a delayed seek cannot seek a track that has since advanced.

Adapter destruction and React integration cleanup only detach subscriptions. Main WebView navigation invalidates queued commands from the old page without stopping playback. Explicit clear/unload, sign-out/session revocation, and process exit still stop playback. Queue state survives WebView recreation within the same native process; restoring a queue after process termination is not implemented. Repeat/shuffle preferences persist in SQLite, with a one-time import from the previous localStorage preferences.

Listening statistics and Last.fm attempt accounting still use `use-listen-tracker.ts`; OS media actions still use browser Media Session. Moving those responsibilities and implementing mobile output/background services are subsequent migration steps. Native audio remains desktop-targeted at this stage.

Tests cover the native queue policy without a frontend, all six shuffle modes, stale index rejection, duplicate completion delivery, repeat boundaries, queue edits while paused, sorted shuffle restoration, native snapshot adoption, reattachment, late seek rejection, and transport/queue revision races. Existing decoder, streaming, presentation-clock, React, and transport tests remain in the full suites. Audible output and mobile background execution require device verification.

See [native audio](native-audio.md) for implementation details and platform limitations.
