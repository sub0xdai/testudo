//! Session-anomaly detector.
//!
//! Flags when the week contains ≥ 2 trades opened outside the user's typical
//! UTC session hours. `baseline.typical_session_hours_utc` is the top-4 hours
//! by trade count over the 30-day baseline; any `opened_at.hour()` not in that
//! set counts as off-hours.
//!
//! Severity:
//! - `Concerning` when off-hours trade count ≥ `CONCERNING_OFF_HOURS_COUNT` (4)
//! - `Notable` otherwise (≥ `OFF_HOURS_COUNT_THRESHOLD`, i.e. 2 or 3)

// @anchor exchange:router:session_anomaly
// @tags api

use std::collections::HashSet;

use chrono::Timelike;
use serde_json::json;
use uuid::Uuid;

use super::super::types::{
    FlaggedPattern, PatternKind, Severity, TradeEvidence, UserBaseline, WeekStats,
};

/// Minimum off-hours trades required to flag at all.
const OFF_HOURS_COUNT_THRESHOLD: usize = 2;
/// Off-hours trades at or above this count escalate to `Concerning`.
const CONCERNING_OFF_HOURS_COUNT: usize = 4;

pub fn detect_session_anomaly(
    baseline: &UserBaseline,
    trades: &[TradeEvidence],
    _stats: &WeekStats,
) -> Option<FlaggedPattern> {
    if baseline.typical_session_hours_utc.is_empty() {
        return None;
    }
    if trades.is_empty() {
        return None;
    }

    let typical: HashSet<u8> = baseline.typical_session_hours_utc.iter().copied().collect();

    let off_hours: Vec<&TradeEvidence> = trades
        .iter()
        .filter(|t| !typical.contains(&(t.opened_at.hour() as u8)))
        .collect();

    if off_hours.len() < OFF_HOURS_COUNT_THRESHOLD {
        return None;
    }

    let severity = if off_hours.len() >= CONCERNING_OFF_HOURS_COUNT {
        Severity::Concerning
    } else {
        Severity::Notable
    };

    let mut anomalous_hours: Vec<u8> = off_hours
        .iter()
        .map(|t| t.opened_at.hour() as u8)
        .collect::<HashSet<u8>>()
        .into_iter()
        .collect();
    anomalous_hours.sort_unstable();

    let mut typical_hours: Vec<u8> = baseline.typical_session_hours_utc.clone();
    typical_hours.sort_unstable();

    let evidence: Vec<Uuid> = off_hours.iter().map(|t| t.id).collect();

    Some(FlaggedPattern {
        pattern: PatternKind::SessionAnomaly,
        severity,
        evidence,
        metrics: json!({
            "off_hours_trade_count": off_hours.len(),
            "typical_session_hours_utc": typical_hours,
            "anomalous_hours_utc": anomalous_hours,
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{Duration, TimeZone, Utc};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use super::*;

    fn base_baseline(typical_hours: Vec<u8>) -> UserBaseline {
        UserBaseline {
            avg_trades_per_day: dec!(1),
            avg_position_size_usd: dec!(1000),
            typical_session_hours_utc: typical_hours,
            win_rate: dec!(0.5),
            avg_r_multiple: dec!(1),
            p90_trades_per_6h: dec!(2),
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

    fn trade_at_hour(hour: u32) -> TradeEvidence {
        let id = Uuid::new_v4();
        let opened = Utc.with_ymd_and_hms(2026, 4, 13, hour, 0, 0).unwrap();
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
    fn fires_when_two_trades_outside_typical_hours() {
        // Typical = 13-16 UTC. Two trades at 03:00 UTC = off-hours.
        let baseline = base_baseline(vec![13, 14, 15, 16]);
        let trades = vec![trade_at_hour(3), trade_at_hour(3), trade_at_hour(14)];

        let result = detect_session_anomaly(&baseline, &trades, &empty_week_stats())
            .expect("expected session anomaly to fire");

        assert_eq!(result.pattern, PatternKind::SessionAnomaly);
        assert_eq!(result.severity, Severity::Notable);
        assert_eq!(result.evidence.len(), 2);
        assert_eq!(result.evidence[0], trades[0].id);
        assert_eq!(result.evidence[1], trades[1].id);
        assert_eq!(result.metrics["off_hours_trade_count"], 2);
    }

    #[test]
    fn does_not_fire_when_all_trades_within_typical_hours() {
        let baseline = base_baseline(vec![13, 14, 15, 16]);
        let trades = vec![
            trade_at_hour(13),
            trade_at_hour(14),
            trade_at_hour(15),
            trade_at_hour(16),
        ];

        assert!(detect_session_anomaly(&baseline, &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn escalates_to_concerning_at_four_off_hours_trades() {
        let baseline = base_baseline(vec![13, 14, 15, 16]);
        let trades = vec![
            trade_at_hour(2),
            trade_at_hour(3),
            trade_at_hour(4),
            trade_at_hour(5),
        ];

        let result = detect_session_anomaly(&baseline, &trades, &empty_week_stats())
            .expect("expected session anomaly to fire");

        assert_eq!(result.severity, Severity::Concerning);
        assert_eq!(result.evidence.len(), 4);
    }

    #[test]
    fn returns_none_when_only_one_trade_is_off_hours() {
        let baseline = base_baseline(vec![13, 14, 15, 16]);
        let trades = vec![
            trade_at_hour(3),
            trade_at_hour(14),
            trade_at_hour(15),
            trade_at_hour(16),
        ];

        assert!(detect_session_anomaly(&baseline, &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn returns_none_when_baseline_has_no_typical_hours() {
        // Cold-start: baseline hasn't identified typical hours yet. Avoid flagging.
        let baseline = base_baseline(Vec::new());
        let trades = vec![trade_at_hour(3), trade_at_hour(4), trade_at_hour(5)];

        assert!(detect_session_anomaly(&baseline, &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn anomalous_hours_metric_deduplicates_and_sorts() {
        let baseline = base_baseline(vec![13, 14, 15, 16]);
        let trades = vec![trade_at_hour(5), trade_at_hour(3), trade_at_hour(5)];

        let result = detect_session_anomaly(&baseline, &trades, &empty_week_stats())
            .expect("expected session anomaly to fire");

        assert_eq!(
            result.metrics["anomalous_hours_utc"],
            serde_json::json!([3, 5])
        );
    }
}
