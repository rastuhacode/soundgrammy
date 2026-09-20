# Native audio streaming

Rust owns the complete playback path: `audio/source.rs` reads cached files or verified ranges from `TrackStream`, `audio/decode.rs` decodes packets with Symphonia, and `audio/output.rs` presents PCM through CPAL. The frontend sends transport commands and receives state; it does not fetch or append audio bytes.

## Download lifecycle and partial cache

- Each native source owns a playback session and cancellation token. Replacing or disposing the source closes the session and prevents new playback downloads. An already in-flight chunk can settle safely.
- Missing ranges use the shared downloader and bounded range cache. Sparse file holes are never treated as downloaded bytes.
- Completed chunks are recorded in a sidecar manifest next to the sparse `.part` file. Reopening restores verified chunks.
- Export and explicit cache operations retain independent download lifetimes.
- Complete files are atomically finalized and subsequent playback opens the cached file directly.
- Active sources protect their files from eviction. Partial playback data remains lowest-priority cache data; explicitly pinned complete files survive normal eviction and TTL cleanup.

## Seeking and buffer display

Seeks reuse the native source, demuxer, decoder worker, and output stream. A latest-wins mailbox cancels obsolete waits, flushes queued PCM, and resets decoding at the new position. Container indexes determine seek accuracy; MP3 landing can be approximate.

A storage-only observer maps downloaded packets to actual timestamps for the buffer display. It does not decode PCM or request downloads. Unmapped data stays unpainted, and cached files show full coverage.

See [native audio](native-audio.md) for implementation details, supported fixtures, and device verification limitations.
