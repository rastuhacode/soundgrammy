# Listen statistics

Append-only listen history + per-track aggregates + likeness score.

## Split

| Layer | Owns |
|-------|------|
| BE `audio/activity.rs` | Attempt lifecycle, rendered PCM time, end reason, Last.fm qualification |
| FE `listen-stats-store` | Hydrated aggregates for smart playlists + live upserts after ends |
| BE `listen_stats` + `db` | Events, aggregates, likeness, rebuild |

Native playback records attempts internally. React receives `listen:stats` updates and reloads aggregates on reattachment. Attempt lifetime follows the native player across UI mounts and unmounts.

The [frontend API](../src/lib/api.ts) exposes aggregate reads, collection settings, rebuild, and clear. See the [Rust command handlers](../src-tauri/src/commands/listen_stats.rs).

Local collection is enabled by default and persisted in `app_settings` under `listen_stats_enabled`. Disabling it pauses local collection and hides Popular and Recent while preserving history. Clearing statistics deletes raw events and aggregates for the active Telegram account, resets the active attempt's local PCM baseline, and invalidates pending writes from before the clear.

## UI consumers

- **Popular** / **Recent** — virtual playlists (`id: popular` / `recent`) of library tracks that have listen history, ordered by likeness / `last_played_at_ms`. Immutable membership; not drag-reorderable.
- **Track info** — Listening section via `get_track_listen_stats` (likeness, plays, skips, listened time, first/last played).

The PCM lifetime counter is independent of seek position and excludes underrun silence. Pauses and temporary interruptions preserve the attempt. Completion, replacement, stop, and output failure close it. The native control worker sends collection settings, clear operations, and attempt edges to one ordered background sink.

## Counting (v1)

- **Effective listened time** uses PCM time capped at the known track duration. Natural completion credits the full known duration. With unknown duration, it uses PCM time.
- **Qualified play** requires a known duration and effective listened time of at least half the track or four minutes, whichever is shorter.
- **Early skip** is an unqualified `skipped`, `replaced`, or `stopped` attempt with less than 30 seconds of effective listened time.
- **Completion** uses the `completed` end reason. Repeat-one opens a new attempt each loop.
- **Likeness** grows logarithmically with qualified plays, listened time, and completions. Early skips lower it, and recent listens boost it. Reads evaluate the recency term against the current time.

The [Rust counting and scoring functions](../src-tauri/src/listen_stats.rs) define the formula and constants.

## Last.fm scrobbling

Last.fm uses its own PCM baseline and settings. Qualification uses actual PCM time, skips tracks of 30 seconds or less, and requires four minutes when duration is unknown. The activity sink delivers start, qualification, and end in order with a process-unique attempt ID. Qualified scrobbles use a durable queue. Now Playing HTTP runs in the delivery worker using the immutable PCM-start timestamp and duration, so it cannot delay local history writes. See [Last.fm qualification and delivery](../src-tauri/src/lastfm/service/playback.rs).

## Tables

- `listen_events` — immutable `attempt_start` / `attempt_end`
- `track_listen_stats` — materialised counters + cached likeness

`listen_track_identity` retains account/document identity after a track is removed. Resaving that document reuses its local ID. Legacy unidentifiable history is retained without guessing its account.

`rebuild_listen_stats` replays end events in bounded pages outside the connection lock, then commits only if history has not changed.
