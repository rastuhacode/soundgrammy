# MSE streaming transport

## Why

Progressive `<audio src="stream:…">` under WKWebView/AVFoundation advertises a fully seekable resource. In practice `audio.buffered` is optimistic and `currentTime` can advance through underruns. The app must feed the decoder only bytes it actually has.

## Implemented transport

| Path | Behavior |
|------|----------|
| Cached track | `asset:` URL via `fileSrc` (unchanged) |
| Streamed track | `MediaSource` + `SourceBuffer.appendBuffer` |
| Bytes | Session-scoped `read_stream_range` / `ensure_stream_range` → `StreamingManager` |
| Progress / buffer UI | Honest `audio.buffered` islands → `buffer-ranges.ts` |
| Seek into unloaded region | MP3: discontinuous island via frame sync. WebM/MP4: grow the existing leading prefix until the target time is covered (clear+rebuild closes MediaSource on WebKit). |
| Unsupported MSE mime | Wait for a cancellable full playback download, then play cached `asset:` (never fall back to progressive `stream:` src) |
| Wrong Telegram MIME | Stream sniffs the file header before MSE attach and corrects container MIME/extension |

## Download lifecycle and partial cache

- Playback fetches only requested byte ranges and keeps about 32 seconds ahead of the playhead.
- Every source load owns a unique backend playback session. Replacing or disposing the source closes that session, so a skip can finish at most already in-flight Telegram requests and cannot schedule more chunks.
- Completed chunks are recorded in a sidecar manifest next to the sparse `.part` file. Reopening the track restores those chunks instead of restarting from byte zero.
- Large ID3v2 tags are skipped for playback startup. After the ledger proves the complete MPEG payload through EOF is local, the app backfills only the backend-validated ID3 prefix, one chunk at a time. A seek island with an audio gap cannot trigger this work, and a skip cancels it.
- Media end-of-stream depends on complete playable MPEG coverage, not on the storage-only ID3 backfill, so unusually large tags cannot stall queue advancement.
- A complete file is atomically finalized and subsequent playback uses the cached file without Telegram requests.
- Partial playback data is lowest-priority cache data. Explicit Cache actions mark complete files as pinned; normal eviction and TTL cleanup do not remove pinned entries.

Primary frontend modules:

- `src/hooks/audio/mse-session.ts` — MediaSource lifecycle + discontinuous `seekToTime`
- `src/hooks/audio/mp3-frame-sync.ts` — MPEG frame sync for island appends
- `src/hooks/audio/mse-append-queue.ts` — append window math
- `src/hooks/audio/use-audio-source.ts` — attaches MSE vs cached
- `src/hooks/audio/use-audio-seek.ts` — buffered vs discontinuous seek

## Limits

- Discontinuous mid-file *island* seeks target `audio/mpeg` (frame sync). WebM/MP4 seek ahead by rebuilding from the leading prefix.
- WKWebView MSE cold start: the media clock can advance ~0.3–0.5s before audible output on each new `MediaSource`. `use-audio-engine` mutes and pins `currentTime` at 0 for 400ms, then unmutes (see `MSE_COLD_START_PRIME_MS`). Cached `asset:` playback is unaffected.
