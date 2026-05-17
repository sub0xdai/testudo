//! Setup-fatigue detector.
//!
//! Flags when a previously-profitable setup degrades sharply during the week.
//! For each setup in `baseline.setup_baselines` with at least
//! `MIN_BASELINE_TRADES` baseline samples and a positive baseline R, compute
//! the week's avg R for that setup. If the week has at least
//! `MIN_WEEK_TRADES` samples and `week_avg_r / baseline_avg_r` falls below
//! `FATIGUE_RATIO_THRESHOLD`, flag the worst-degraded setup (lowest ratio).
//!
//! Severity:
//! - `Concerning` when ratio ≤ `CONCERNING_RATIO_THRESHOLD` (0.25)
//! - `Notable` otherwise (ratio < 0.5)
//!
//! Note: the spec wording is "trailing 10 trades across baseline+week" but
//! the digest input carries only aggregated baselines (no raw baseline
//! trades). MVP uses the week's trades for that setup as the recency signal,
//! gated by `MIN_WEEK_TRADES` so a single bad trade can't trigger fatigue.
//! Untagged trades are excluded — fatigue is a property of a specific setup.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use uuid::Uuid;

use super::super::types::{
    FlaggedPattern, PatternKind, Severity, TradeEvidence, UserBaseline, WeekStats,
};

/// Setups need this many baseline trades before they're eligible for fatigue analysis.
const MIN_BASELINE_TRADES: i64 = 5;
/// Minimum week trades for a setup to be considered for fatigue.
const MIN_WEEK_TRADES: usize = 3;
/// Week-avg-R / baseline-avg-R ratio at or below this fires the flag.
const FATIGUE_RATIO_THRESHOLD: Decimal = dec!(0.5);
/// Ratios at or below this escalate to `Concerning`.
const CONCERNING_RATIO_THRESHOLD: Decimal = dec!(0.25);
/// Bucket key reserved for trades without a setup tag — excluded from fatigue.
const UNTAGGED_KEY: &str = "(untagged)";

pub fn detect_setup_fatigue(
    baseline: &UserBaseline,
    trades: &[TradeEvidence],
    _stats: &WeekStats,
) -> Option<FlaggedPattern> {
    if baseline.setup_baselines.is_empty() || trades.is_empty() {
        return None;
    }

    let mut worst: Option<(String, Decimal, Decimal, Decimal, Vec<Uuid>)> = None;

    for (setup_key, setup_baseline) in &baseline.setup_baselines {
        if setup_key == UNTAGGED_KEY {
            continue;
        }
        if setup_baseline.trade_count < MIN_BASELINE_TRADES {
            continue;
        }
        if setup_baseline.avg_r_multiple <= Decimal::ZERO {
            continue;
        }

        let week_trades: Vec<&TradeEvidence> = trades
            .iter()
            .filter(|t| {
                t.setup_tag
                    .as_deref()
                    .map(|tag| tag.to_lowercase() == *setup_key)
                    .unwrap_or(false)
            })
            .collect();

        if week_trades.len() < MIN_WEEK_TRADES {
            continue;
        }

        let r_sum: Decimal = week_trades
            .iter()
            .filter_map(|t| t.r_multiple)
            .sum();
        let r_count = week_trades
            .iter()
            .filter(|t| t.r_multiple.is_some())
            .count();

        if r_count < MIN_WEEK_TRADES {
            continue;
        }

        let week_avg_r = r_sum / Decimal::from(r_count as i64);
        let ratio = week_avg_r / setup_baseline.avg_r_multiple;

        if ratio >= FATIGUE_RATIO_THRESHOLD {
            continue;
        }

        let evidence: Vec<Uuid> = week_trades.iter().map(|t| t.id).collect();
        let candidate = (
            setup_key.clone(),
            ratio,
            week_avg_r,
            setup_baseline.avg_r_multiple,
            evidence,
        );

        match &worst {
            Some((_, worst_ratio, _, _, _)) if *worst_ratio <= ratio => {}
            _ => worst = Some(candidate),
        }
    }

    let (setup, ratio, week_avg_r, baseline_avg_r, evidence) = worst?;

    let severity = if ratio <= CONCERNING_RATIO_THRESHOLD {
        Severity::Concerning
    } else {
        Severity::Notable
    };

    Some(FlaggedPattern {
        pattern: PatternKind::SetupFatigue,
        severity,
        evidence,
        metrics: json!({
            "setup": setup,
            "fatigue_ratio": ratio.to_string(),
            "week_avg_r": week_avg_r.to_string(),
            "baseline_avg_r": baseline_avg_r.to_string(),
            "min_baseline_trades": MIN_BASELINE_TRADES,
            "min_week_trades": MIN_WEEK_TRADES,
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{Duration, TimeZone, Utc};
    use rust_decimal_macros::dec;

    use super::super::super::types::SetupBaseline;
    use super::*;

    fn baseline_with_setups(setups: Vec<(&str, i64, Decimal)>) -> UserBaseline {
        let mut setup_baselines = HashMap::new();
        for (name, count, avg_r) in setups {
            setup_baselines.insert(
                name.to_string(),
                SetupBaseline {
                    trade_count: count,
                    avg_r_multiple: avg_r,
                    win_rate: dec!(0.5),
                },
            );
        }
        UserBaseline {
            avg_trades_per_day: dec!(1),
            avg_position_size_usd: dec!(1000),
            typical_session_hours_utc: vec![13, 14, 15, 16],
            win_rate: dec!(0.5),
            avg_r_multiple: dec!(1),
            p90_trades_per_6h: dec!(2),
            setup_baselines,
        }
    }

    fn empty_week_stats() -> WeekStats {
        WeekStats {
            trade_count: 0,
            win_rate: Decimal::ZERO,
            total_pnl: Decimal::ZERO,
            total_r: Decimal::ZERO,
            trades_by_hour_utc: [0; 24],
            by_setup: HashMap::new(),
        }
    }

    fn trade_with_setup(hour: i64, r: Decimal, setup: Option<&str>) -> TradeEvidence {
        let id = Uuid::new_v4();
        let opened = Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap()
            + Duration::hours(hour);
        TradeEvidence {
            id,
            short_id: id.simple().to_string().chars().take(8).collect(),
            symbol: "BTC_USDT".to_string(),
            side: "long".to_string(),
            opened_at: opened,
            closed_at: opened + Duration::hours(1),
            pnl: r * dec!(100),
            r_multiple: Some(r),
            setup_tag: setup.map(String::from),
            position_size_usd: dec!(1000),
        }
    }

    #[test]
    fn fires_when_setup_week_r_falls_below_half_baseline() {
        // breakout baseline avg R = 1.2 over 10 trades. Week's 3 trades avg R = 0.4 → ratio 0.333 → Notable.
        let baseline = baseline_with_setups(vec![("breakout", 10, dec!(1.2))]);
        let trades = vec![
            trade_with_setup(0, dec!(0.4), Some("breakout")),
            trade_with_setup(1, dec!(0.4), Some("breakout")),
            trade_with_setup(2, dec!(0.4), Some("breakout")),
        ];

        let result = detect_setup_fatigue(&baseline, &trades, &empty_week_stats())
            .expect("expected setup fatigue to fire");

        assert_eq!(result.pattern, PatternKind::SetupFatigue);
        assert_eq!(result.severity, Severity::Notable);
        assert_eq!(result.evidence.len(), 3);
        assert_eq!(result.metrics["setup"], "breakout");
    }

    #[test]
    fn does_not_fire_when_week_r_close_to_baseline() {
        let baseline = baseline_with_setups(vec![("breakout", 10, dec!(1.2))]);
        let trades = vec![
            trade_with_setup(0, dec!(1.0), Some("breakout")),
            trade_with_setup(1, dec!(1.1), Some("breakout")),
            trade_with_setup(2, dec!(0.9), Some("breakout")),
        ];

        assert!(detect_setup_fatigue(&baseline, &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn escalates_severity_at_quarter_baseline_ratio() {
        // Week avg R = 0.2 vs baseline 1.2 → ratio 0.166 → Concerning.
        let baseline = baseline_with_setups(vec![("breakout", 10, dec!(1.2))]);
        let trades = vec![
            trade_with_setup(0, dec!(0.2), Some("breakout")),
            trade_with_setup(1, dec!(0.2), Some("breakout")),
            trade_with_setup(2, dec!(0.2), Some("breakout")),
        ];

        let result = detect_setup_fatigue(&baseline, &trades, &empty_week_stats())
            .expect("expected setup fatigue to fire");

        assert_eq!(result.severity, Severity::Concerning);
    }

    #[test]
    fn skips_setups_with_insufficient_baseline_trades() {
        // breakout has only 4 baseline trades — below MIN_BASELINE_TRADES (5).
        let baseline = baseline_with_setups(vec![("breakout", 4, dec!(1.2))]);
        let trades = vec![
            trade_with_setup(0, dec!(0.1), Some("breakout")),
            trade_with_setup(1, dec!(0.1), Some("breakout")),
            trade_with_setup(2, dec!(0.1), Some("breakout")),
        ];

        assert!(detect_setup_fatigue(&baseline, &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn requires_minimum_week_trades_for_setup() {
        // Only 2 week trades — one bad trade shouldn't trigger fatigue.
        let baseline = baseline_with_setups(vec![("breakout", 10, dec!(1.2))]);
        let trades = vec![
            trade_with_setup(0, dec!(0.1), Some("breakout")),
            trade_with_setup(1, dec!(0.1), Some("breakout")),
        ];

        assert!(detect_setup_fatigue(&baseline, &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn ignores_untagged_trades() {
        // (untagged) bucket exists in baseline but should never flag fatigue.
        let baseline = baseline_with_setups(vec![("(untagged)", 20, dec!(1.5))]);
        let trades = vec![
            trade_with_setup(0, dec!(0.1), None),
            trade_with_setup(1, dec!(0.1), None),
            trade_with_setup(2, dec!(0.1), None),
        ];

        assert!(detect_setup_fatigue(&baseline, &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn picks_worst_setup_when_multiple_qualify() {
        // breakout ratio = 0.333, fade ratio = 0.1 → fade is worse, fade should be picked.
        let baseline = baseline_with_setups(vec![
            ("breakout", 10, dec!(1.2)),
            ("fade", 10, dec!(2.0)),
        ]);
        let trades = vec![
            trade_with_setup(0, dec!(0.4), Some("breakout")),
            trade_with_setup(1, dec!(0.4), Some("breakout")),
            trade_with_setup(2, dec!(0.4), Some("breakout")),
            trade_with_setup(3, dec!(0.2), Some("fade")),
            trade_with_setup(4, dec!(0.2), Some("fade")),
            trade_with_setup(5, dec!(0.2), Some("fade")),
        ];

        let result = detect_setup_fatigue(&baseline, &trades, &empty_week_stats())
            .expect("expected setup fatigue to fire");

        assert_eq!(result.metrics["setup"], "fade");
        assert_eq!(result.severity, Severity::Concerning);
    }

    #[test]
    fn matches_setup_tag_case_insensitively() {
        let baseline = baseline_with_setups(vec![("breakout", 10, dec!(1.2))]);
        let trades = vec![
            trade_with_setup(0, dec!(0.3), Some("Breakout")),
            trade_with_setup(1, dec!(0.3), Some("BREAKOUT")),
            trade_with_setup(2, dec!(0.3), Some("breakout")),
        ];

        let result = detect_setup_fatigue(&baseline, &trades, &empty_week_stats())
            .expect("expected setup fatigue to fire");

        assert_eq!(result.metrics["setup"], "breakout");
        assert_eq!(result.evidence.len(), 3);
    }
}
