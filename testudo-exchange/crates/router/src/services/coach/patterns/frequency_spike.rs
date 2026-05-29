//! Frequency-spike detector.
//!
//! Flags when the week contains a 6h window whose trade count materially
//! exceeds the user's baseline 90th-percentile 6h window. Rule: for every
//! trade `i` in the week, count trades (including `i`) whose `opened_at`
//! falls within `[i.opened_at, i.opened_at + 6h)`. The max across all
//! anchors is the week's "peak window count." When
//! `peak > SPIKE_MULTIPLIER_THRESHOLD × baseline.p90_trades_per_6h`, flag.
//!
//! Severity:
//! - `Concerning` when the multiplier is ≥ `CONCERNING_MULTIPLIER_THRESHOLD` (2.5×)
//! - `Notable` otherwise (strictly > 1.5×)

// @anchor exchange:router:frequency_spike
// @tags api

use chrono::Duration;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use uuid::Uuid;

use super::super::types::{
    FlaggedPattern, PatternKind, Severity, TradeEvidence, UserBaseline, WeekStats,
};

/// Peak window must exceed 1.5× the baseline p90 to flag.
const SPIKE_MULTIPLIER_THRESHOLD: Decimal = dec!(1.5);
/// 2.5× baseline p90 escalates the flag to `Concerning`.
const CONCERNING_MULTIPLIER_THRESHOLD: Decimal = dec!(2.5);
/// Rolling window size.
const WINDOW_HOURS: i64 = 6;

pub fn detect_frequency_spike(
    baseline: &UserBaseline,
    trades: &[TradeEvidence],
    _stats: &WeekStats,
) -> Option<FlaggedPattern> {
    if baseline.p90_trades_per_6h <= Decimal::ZERO {
        return None;
    }
    if trades.is_empty() {
        return None;
    }

    let mut sorted: Vec<&TradeEvidence> = trades.iter().collect();
    sorted.sort_by_key(|t| t.opened_at);

    let window = Duration::hours(WINDOW_HOURS);
    let mut best_window: Vec<&TradeEvidence> = Vec::new();

    for (i, anchor) in sorted.iter().enumerate() {
        let end = anchor.opened_at + window;
        let count = sorted[i..]
            .iter()
            .take_while(|t| t.opened_at < end)
            .count();
        if count > best_window.len() {
            best_window = sorted[i..i + count].to_vec();
        }
    }

    let peak = Decimal::from(best_window.len() as i64);
    let multiplier = peak / baseline.p90_trades_per_6h;

    if multiplier <= SPIKE_MULTIPLIER_THRESHOLD {
        return None;
    }

    let severity = if multiplier >= CONCERNING_MULTIPLIER_THRESHOLD {
        Severity::Concerning
    } else {
        Severity::Notable
    };

    let window_start = best_window.first().map(|t| t.opened_at.to_rfc3339());
    let window_end = best_window.last().map(|t| t.opened_at.to_rfc3339());
    let evidence: Vec<Uuid> = best_window.iter().map(|t| t.id).collect();

    Some(FlaggedPattern {
        pattern: PatternKind::FrequencySpike,
        severity,
        evidence,
        metrics: json!({
            "peak_window_count": best_window.len(),
            "baseline_p90_trades_per_6h": baseline.p90_trades_per_6h.to_string(),
            "multiplier": multiplier.to_string(),
            "window_hours": WINDOW_HOURS,
            "window_start": window_start,
            "window_end": window_end,
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{TimeZone, Utc};

    use super::*;

    fn base_baseline(p90: Decimal) -> UserBaseline {
        UserBaseline {
            avg_trades_per_day: dec!(1),
            avg_position_size_usd: dec!(1000),
            typical_session_hours_utc: vec![13, 14, 15, 16],
            win_rate: dec!(0.5),
            avg_r_multiple: dec!(1),
            p90_trades_per_6h: p90,
            setup_baselines: HashMap::new(),
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

    fn trade_at(hour: i64, minute: i64) -> TradeEvidence {
        let id = Uuid::new_v4();
        let opened = Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap()
            + Duration::hours(hour)
            + Duration::minutes(minute);
        TradeEvidence {
            id,
            short_id: id.simple().to_string().chars().take(8).collect(),
            symbol: "BTC_USDT".to_string(),
            side: "long".to_string(),
            opened_at: opened,
            closed_at: opened + Duration::hours(1),
            pnl: Decimal::ZERO,
            r_multiple: None,
            setup_tag: None,
            position_size_usd: dec!(1000),
        }
    }

    #[test]
    fn fires_when_peak_window_exceeds_baseline_p90() {
        // Baseline p90 = 2. Six trades crammed into a 4h afternoon window.
        let baseline = base_baseline(dec!(2));
        let trades = vec![
            trade_at(13, 0),
            trade_at(13, 30),
            trade_at(14, 0),
            trade_at(14, 45),
            trade_at(15, 30),
            trade_at(16, 15),
        ];

        let result = detect_frequency_spike(&baseline, &trades, &empty_week_stats())
            .expect("expected frequency spike to fire");

        assert_eq!(result.pattern, PatternKind::FrequencySpike);
        assert_eq!(result.severity, Severity::Concerning);
        assert_eq!(result.evidence.len(), 6);
        assert_eq!(result.evidence[0], trades[0].id);
        assert_eq!(result.metrics["peak_window_count"], 6);
        assert_eq!(
            result.metrics["multiplier"],
            serde_json::Value::String("3".to_string())
        );
    }

    #[test]
    fn does_not_fire_when_week_is_evenly_spaced() {
        // Baseline p90 = 3. Seven trades spread one per day — peak window = 1.
        let baseline = base_baseline(dec!(3));
        let trades: Vec<TradeEvidence> =
            (0..7).map(|d| trade_at(d * 24 + 13, 0)).collect();

        assert!(detect_frequency_spike(&baseline, &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn escalates_severity_only_at_large_multiplier() {
        // Baseline p90 = 2. Four trades in one window → peak=4, multiplier=2
        // (below concerning threshold 2.5) → Notable.
        let baseline = base_baseline(dec!(2));
        let trades = vec![
            trade_at(13, 0),
            trade_at(13, 30),
            trade_at(14, 0),
            trade_at(14, 30),
        ];

        let result = detect_frequency_spike(&baseline, &trades, &empty_week_stats())
            .expect("expected frequency spike to fire");

        assert_eq!(result.severity, Severity::Notable);
        assert_eq!(result.evidence.len(), 4);
    }

    #[test]
    fn returns_none_when_baseline_p90_is_zero() {
        // Cold-start user with empty baseline — never flag.
        let baseline = base_baseline(Decimal::ZERO);
        let trades = vec![
            trade_at(13, 0),
            trade_at(13, 30),
            trade_at(14, 0),
            trade_at(14, 45),
        ];

        assert!(detect_frequency_spike(&baseline, &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn peak_window_respects_6h_boundary() {
        // Baseline p90 = 1. Four trades spread across 8h — no 6h window
        // contains more than 3. Multiplier 3/1 = 3 → Concerning.
        let baseline = base_baseline(dec!(1));
        let trades = vec![
            trade_at(10, 0),
            trade_at(12, 0),
            trade_at(14, 0),
            trade_at(18, 0), // 8h later — outside 10:00's window.
        ];

        let result = detect_frequency_spike(&baseline, &trades, &empty_week_stats())
            .expect("expected frequency spike to fire");

        assert_eq!(result.evidence.len(), 3);
        assert_eq!(result.evidence[0], trades[0].id);
        assert_eq!(result.evidence[2], trades[2].id);
    }
}
