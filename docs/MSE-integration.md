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
| Seek into unloaded region | Discontinuous MSE: clear buffer, MP3 frame sync near target byte, `timestampOffset`, append island, grow forward |
| Unsupported MSE mime | Wait for full `download_track`, then play cached `asset:` (never fall back to progressive `stream:` src) |

Primary frontend modules:

- `src/hooks/audio/mse-session.ts` — MediaSource lifecycle + discontinuous `seekToTime`
- `src/hooks/audio/mp3-frame-sync.ts` — MPEG frame sync for island appends
- `src/hooks/audio/mse-append-queue.ts` — append window math
- `src/hooks/audio/use-audio-source.ts` — attaches MSE vs cached
- `src/hooks/audio/use-audio-seek.ts` — buffered vs discontinuous seek

## Limits

- Discontinuous seeks target `audio/mpeg` (frame sync). Other codecs may need remux (fMP4) later.
- OS Now Playing and a native Rust decoder remain out of scope.
