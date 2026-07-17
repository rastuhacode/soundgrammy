//! Listen statistics: qualification, early-skip, aggregates, and likeness.
//!
//! Business rules live in `docs/listen-statistics.md` and `__local/listen-statistics.md`.

use serde::{Deserialize, Serialize};

/// Early-skip threshold (30 seconds).
pub const T_EARLY_MS: i64 = 30_000;
/// Time normalization unit for likeness (~4 minutes).
pub const T0_MS: i64 = 240_000;
/// Classic qualification: half the track, capped at 4 minutes.
pub const QUALIFY_CAP_MS: i64 = 240_000;

pub const ALPHA: f64 = 1.0;
pub const BETA: f64 = 1.0;
pub const GAMMA: f64 = 0.5;
pub const DELTA: f64 = 1.5;
pub const EPSILON: f64 = 0.25;
/// Recency scale τ in days (R = exp(-Δ/τ)).
pub const TAU_DAYS: f64 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    Completed,
    Skipped,
    Replaced,
    Stopped,
    Interrupted,
}

impl EndReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Skipped => "skipped",
            Self::Replaced => "replaced",
            Self::Stopped => "stopped",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "completed" => Some(Self::Completed),
            "skipped" => Some(Self::Skipped),
            "replaced" => Some(Self::Replaced),
            "stopped" => Some(Self::Stopped),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }

    pub fn is_abandonment(self) -> bool {
        matches!(self, Self::Skipped | Self::Replaced | Self::Stopped)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ListenAggregates {
    pub starts: i64,
    pub qualified_plays: i64,
    pub completes: i64,
    pub early_skips: i64,
    pub total_listened_ms: i64,
    pub first_played_at_ms: Option<i64>,
    pub last_played_at_ms: Option<i64>,
}

impl ListenAggregates {
    #[allow(dead_code)]
    pub fn rate_complete(self) -> f64 {
        if self.starts > 0 {
            self.completes as f64 / self.starts as f64
        } else {
            0.0
        }
    }

    pub fn rate_early(self) -> f64 {
        if self.starts > 0 {
            self.early_skips as f64 / self.starts as f64
        } else {
            0.0
        }
    }

    #[allow(dead_code)]
    pub fn rate_qual(self) -> f64 {
        if self.starts > 0 {
            self.qualified_plays as f64 / self.starts as f64
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AttemptEndInput {
    pub listened_ms: i64,
    pub duration_ms: Option<i64>,
    pub end_reason: EndReason,
    /// Wall-clock end time; applied in `apply_attempt_end`, not in derive.
    #[allow(dead_code)]
    pub ended_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttemptEndDerived {
    pub listened_eff_ms: i64,
    pub qualified: bool,
    pub early_skip: bool,
}

/// Effective listened time: min(L, D) when D known; on complete, L_eff = D.
pub fn effective_listened_ms(
    listened_ms: i64,
    duration_ms: Option<i64>,
    end_reason: EndReason,
) -> i64 {
    let listened = listened_ms.max(0);
    match (end_reason, duration_ms) {
        (EndReason::Completed, Some(d)) if d > 0 => d,
        (_, Some(d)) if d > 0 => listened.min(d),
        _ => listened,
    }
}

/// Classic rule: L_eff >= min(D/2, 4 min). Unknown/non-positive D → not qualified.
pub fn is_qualified(listened_eff_ms: i64, duration_ms: Option<i64>) -> bool {
    let Some(d) = duration_ms.filter(|&d| d > 0) else {
        return false;
    };
    let threshold = (d / 2).min(QUALIFY_CAP_MS);
    listened_eff_ms >= threshold
}

/// Early skip: abandonment + L_eff < 30s, and not qualified.
pub fn is_early_skip(
    end_reason: EndReason,
    listened_eff_ms: i64,
    qualified: bool,
) -> bool {
    if qualified || !end_reason.is_abandonment() {
        return false;
    }
    listened_eff_ms < T_EARLY_MS
}

pub fn derive_attempt_end(input: AttemptEndInput) -> AttemptEndDerived {
    let listened_eff_ms =
        effective_listened_ms(input.listened_ms, input.duration_ms, input.end_reason);
    let qualified = is_qualified(listened_eff_ms, input.duration_ms);
    let early_skip = is_early_skip(input.end_reason, listened_eff_ms, qualified);
    AttemptEndDerived {
        listened_eff_ms,
        qualified,
        early_skip,
    }
}

pub fn apply_attempt_end(
    mut agg: ListenAggregates,
    derived: AttemptEndDerived,
    end_reason: EndReason,
    ended_at_ms: i64,
) -> ListenAggregates {
    agg.starts += 1;
    if derived.qualified {
        agg.qualified_plays += 1;
    }
    if end_reason == EndReason::Completed {
        agg.completes += 1;
    }
    if derived.early_skip {
        agg.early_skips += 1;
    }
    agg.total_listened_ms += derived.listened_eff_ms;
    if agg.first_played_at_ms.is_none() {
        agg.first_played_at_ms = Some(ended_at_ms);
    }
    agg.last_played_at_ms = Some(ended_at_ms);
    agg
}

/// Likeness λ from aggregates + current time (ms since Unix epoch).
pub fn compute_likeness(agg: &ListenAggregates, now_ms: i64) -> f64 {
    if agg.starts <= 0 {
        return 0.0;
    }

    let p = ALPHA * (1.0 + agg.qualified_plays as f64).ln()
        + BETA * (1.0 + agg.total_listened_ms as f64 / T0_MS as f64).ln()
        + GAMMA * (1.0 + agg.completes as f64).ln();

    let e = DELTA * agg.rate_early();

    let r = match agg.last_played_at_ms {
        Some(last) => {
            let delta_days = (now_ms.saturating_sub(last) as f64) / (86_400_000.0);
            (-delta_days / TAU_DAYS).exp()
        }
        None => 0.0,
    };

    // Spec: λ = max(0, (P − E)(1 + εR))
    ((p - e) * (1.0 + EPSILON * r)).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_half_of_short_track() {
        // D = 60s → threshold 30s
        assert!(is_qualified(30_000, Some(60_000)));
        assert!(!is_qualified(29_999, Some(60_000)));
    }

    #[test]
    fn qualified_caps_at_four_minutes() {
        // D = 20 min → threshold 4 min
        assert!(is_qualified(240_000, Some(1_200_000)));
        assert!(!is_qualified(239_999, Some(1_200_000)));
    }

    #[test]
    fn unknown_duration_not_qualified() {
        assert!(!is_qualified(120_000, None));
        assert!(!is_qualified(120_000, Some(0)));
        assert!(!is_qualified(120_000, Some(-1)));
    }

    #[test]
    fn completed_sets_leff_to_duration() {
        let d = effective_listened_ms(5_000, Some(180_000), EndReason::Completed);
        assert_eq!(d, 180_000);
        assert!(is_qualified(d, Some(180_000)));
    }

    #[test]
    fn early_skip_abandonment_under_30s() {
        assert!(is_early_skip(EndReason::Skipped, 10_000, false));
        assert!(is_early_skip(EndReason::Replaced, 0, false));
        assert!(is_early_skip(EndReason::Stopped, 29_999, false));
    }

    #[test]
    fn early_skip_excludes_interrupted_and_completed() {
        assert!(!is_early_skip(EndReason::Interrupted, 5_000, false));
        assert!(!is_early_skip(EndReason::Completed, 5_000, false));
    }

    #[test]
    fn qualified_wins_mutual_exclusion() {
        // Short track: D=40s, L_eff=20s → qualified (half), not early skip
        let derived = derive_attempt_end(AttemptEndInput {
            listened_ms: 20_000,
            duration_ms: Some(40_000),
            end_reason: EndReason::Skipped,
            ended_at_ms: 1,
        });
        assert!(derived.qualified);
        assert!(!derived.early_skip);
    }

    #[test]
    fn never_played_likeness_zero() {
        assert_eq!(compute_likeness(&ListenAggregates::default(), 1_000), 0.0);
    }

    #[test]
    fn more_qualified_does_not_decrease_likeness() {
        let now = 1_000_000_i64;
        let base = ListenAggregates {
            starts: 2,
            qualified_plays: 1,
            completes: 0,
            early_skips: 0,
            total_listened_ms: 60_000,
            first_played_at_ms: Some(now),
            last_played_at_ms: Some(now),
        };
        let more_q = ListenAggregates {
            qualified_plays: 2,
            ..base
        };
        assert!(compute_likeness(&more_q, now) >= compute_likeness(&base, now));
    }

    #[test]
    fn likeness_never_negative_when_early_dominates() {
        let now = 1_000_000_i64;
        // Many early skips, almost no positive mass → P − E < 0 before clamp.
        let agg = ListenAggregates {
            starts: 10,
            qualified_plays: 0,
            completes: 0,
            early_skips: 10,
            total_listened_ms: 0,
            first_played_at_ms: Some(now),
            last_played_at_ms: Some(now),
        };
        assert!(compute_likeness(&agg, now) >= 0.0);
    }

    #[test]
    fn more_early_rate_does_not_increase_likeness() {
        let now = 1_000_000_i64;
        let low_early = ListenAggregates {
            starts: 10,
            qualified_plays: 5,
            completes: 2,
            early_skips: 1,
            total_listened_ms: 600_000,
            first_played_at_ms: Some(now),
            last_played_at_ms: Some(now),
        };
        let high_early = ListenAggregates {
            early_skips: 8,
            ..low_early
        };
        assert!(compute_likeness(&high_early, now) <= compute_likeness(&low_early, now));
    }

    #[test]
    fn apply_attempt_end_updates_counts() {
        let derived = AttemptEndDerived {
            listened_eff_ms: 120_000,
            qualified: true,
            early_skip: false,
        };
        let agg = apply_attempt_end(
            ListenAggregates::default(),
            derived,
            EndReason::Completed,
            5_000,
        );
        assert_eq!(agg.starts, 1);
        assert_eq!(agg.qualified_plays, 1);
        assert_eq!(agg.completes, 1);
        assert_eq!(agg.early_skips, 0);
        assert_eq!(agg.total_listened_ms, 120_000);
        assert_eq!(agg.first_played_at_ms, Some(5_000));
        assert_eq!(agg.last_played_at_ms, Some(5_000));
    }
}
