# Listen statistics

Append-only listen history + per-track aggregates + likeness score. No UI.

## Split

| Layer | Owns |
|-------|------|
| FE `use-listen-tracker` | Attempt lifecycle, wall-clock playing time, end reason |
| BE `listen_stats` + `db` | Events, aggregates, likeness, rebuild |

IPC: `record_listen_start` / `record_listen_end` / `get_track_listen_stats` / `list_listen_stats` / `rebuild_listen_stats` via [`src/lib/api.ts`](../src/lib/api.ts).

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
