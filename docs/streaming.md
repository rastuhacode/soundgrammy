# MSE streaming transport

## Why

Progressive `<audio src="stream:…">` under WKWebView/AVFoundation advertises a fully seekable resource. In practice `audio.buffered` is optimistic and `currentTime` can advance through underruns. The app must feed the decoder only bytes it actually has.

## Implemented transport

| Path | Behavior |
|------|----------|
| Cached track | `asset:` URL via `fileSrc` (unchanged) |
| Streamed track | `MediaSource` + `SourceBuffer.appendBuffer` |
| Bytes | `read_stream_range` / `ensure_stream_range` → `StreamingManager` |
| Progress / buffer UI | Honest `audio.buffered` islands → `buffer-ranges.ts` |
| Seek into unloaded region | MP3: discontinuous island via frame sync. WebM/MP4: grow the existing leading prefix until the target time is covered (clear+rebuild closes MediaSource on WebKit). |
| Unsupported MSE mime | Wait for full `download_track`, then play cached `asset:` (never fall back to progressive `stream:` src) |
| Wrong Telegram MIME | Stream sniffs the file header before MSE attach and corrects container MIME/extension |

Primary frontend modules:

- `src/hooks/audio/mse-session.ts` — MediaSource lifecycle + discontinuous `seekToTime`
- `src/hooks/audio/mp3-frame-sync.ts` — MPEG frame sync for island appends
- `src/hooks/audio/mse-append-queue.ts` — append window math
- `src/hooks/audio/use-audio-source.ts` — attaches MSE vs cached
- `src/hooks/audio/use-audio-seek.ts` — buffered vs discontinuous seek

## Limits

- Discontinuous mid-file *island* seeks target `audio/mpeg` (frame sync). WebM/MP4 seek ahead by rebuilding from the leading prefix.
