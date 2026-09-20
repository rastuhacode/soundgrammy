# Listen statistics

Append-only listen history + per-track aggregates + likeness score.

## Split

| Layer | Owns |
|-------|------|
| BE `audio/activity.rs` | Attempt lifecycle, rendered PCM time, end reason, Last.fm qualification |
| FE `listen-stats-store` | Hydrated aggregates for smart playlists + live upserts after ends |
| BE `listen_stats` + `db` | Events, aggregates, likeness, rebuild |

Native playback records attempts directly. React receives `listen:stats` updates and reloads aggregates on reattachment; mounting/unmounting never opens or closes an attempt. Compatibility write commands remain available, but the production player does not call them.

Read/settings IPC: `get_track_listen_stats` / `list_listen_stats` / `rebuild_listen_stats` / `clear_listen_statistics` via [`src/lib/api.ts`](../src/lib/api.ts).

Collection is enabled by default and persisted in `app_settings` under `listen_stats_enabled`. When disabled, the backend ignores listen attempts and the UI hides Popular and Recent without deleting existing history. Clearing statistics deletes both raw events and aggregates; the native attempt baseline restarts so listening before the clear boundary is not re-added later.

## UI consumers

- **Popular** / **Recent** — virtual playlists (`id: popular` / `recent`) of library tracks that have listen history, ordered by likeness / `last_played_at_ms`. Immutable membership; not drag-reorderable.
- **Track info** — Listening section via `get_track_listen_stats` (likeness, plays, skips, listened time, first/last played).

The PCM lifetime counter is independent of seek position and excludes underrun silence. Pauses and temporary interruptions preserve the attempt; completion, replacement, stop, and output failure close it. Statistics clear/enable commands run on the native control worker, serialized with attempt ends. Last.fm start, qualification, and end are delivered in order by a single native task, using process-unique IDs and the existing durable scrobble queue.

## Counting (v1)

- **Qualified play:** \(L_{\mathrm{eff}} \ge \min(D/2,\ 4\,\mathrm{min})\). Unknown \(D\) → not qualified.
- **Early skip:** abandonment (`skipped`/`replaced`/`stopped`) and \(L_{\mathrm{eff}} < 30\,\mathrm{s}\). Not `interrupted`/`completed`. If qualified, not early skip.
- **Complete:** end reason `completed` (repeat-one = new attempt per loop).
- **Likeness:** log growth from qualified / listened time / completes, minus early-skip rate, mild recency uplift. Recompute on aggregate change; refresh with “now” on read. Defaults: α=β=1, γ=0.5, δ=1.5, ε=0.25, τ=90d.

Full formula: `__local/listen-statistics.md`.

## Tables

- `listen_events` — immutable `attempt_start` / `attempt_end`
- `track_listen_stats` — materialised counters + cached likeness

`rebuild_listen_stats` replays all end events into aggregates.
