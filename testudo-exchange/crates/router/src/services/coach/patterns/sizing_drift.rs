//! Sizing-drift detector.
//!
//! Flags when a trader enlarges their position after losses — the classic
//! revenge-sizing pattern. Rule: of the week's post-loss trades (a trade
//! whose immediate predecessor was a loss), the last `POST_LOSS_WINDOW`
//! must exist and their average `position_size_usd` must exceed
//! `DRIFT_MULTIPLIER_THRESHOLD` × `baseline.avg_position_size_usd`.
//!
//! Severity:
//! - `Concerning` when the multiplier is ≥ `CONCERNING_MULTIPLIER_THRESHOLD` (2.5×)
//! - `Notable` otherwise (strictly > 1.5×)

// @anchor exchange:router:sizing_drift
// @tags api

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde_json::json;
use uuid::Uuid;

use super::super::types::{
    FlaggedPattern, PatternKind, Severity, TradeEvidence, UserBaseline, WeekStats,
};

/// Post-loss trade sizes exceeding 1.5× baseline trigger the flag.
const DRIFT_MULTIPLIER_THRESHOLD: Decimal = dec!(1.5);
/// 2.5× baseline escalates the flag to `Concerning`.
const CONCERNING_MULTIPLIER_THRESHOLD: Decimal = dec!(2.5);
/// How many most-recent post-loss trades feed the multiplier.
const POST_LOSS_WINDOW: usize = 3;

pub fn detect_sizing_drift(
    baseline: &UserBaseline,
    trades: &[TradeEvidence],
    _stats: &WeekStats,
) -> Option<FlaggedPattern> {
    if baseline.avg_position_size_usd <= Decimal::ZERO {
        return None;
    }

    let mut sorted: Vec<&TradeEvidence> = trades.iter().collect();
    sorted.sort_by_key(|t| t.opened_at);

    let post_loss: Vec<&TradeEvidence> = sorted
        .windows(2)
        .filter_map(|w| (w[0].pnl < Decimal::ZERO).then_some(w[1]))
        .collect();

    if post_loss.len() < POST_LOSS_WINDOW {
        return None;
    }

    let tail = &post_loss[post_loss.len() - POST_LOSS_WINDOW..];
    let sum: Decimal = tail.iter().map(|t| t.position_size_usd).sum();
    let avg_size = sum / Decimal::from(POST_LOSS_WINDOW as i64);
    let multiplier = avg_size / baseline.avg_position_size_usd;

    if multiplier <= DRIFT_MULTIPLIER_THRESHOLD {
        return None;
    }

    let severity = if multiplier >= CONCERNING_MULTIPLIER_THRESHOLD {
        Severity::Concerning
    } else {
        Severity::Notable
    };

    let evidence: Vec<Uuid> = tail.iter().map(|t| t.id).collect();

    Some(FlaggedPattern {
        pattern: PatternKind::SizingDrift,
        severity,
        evidence,
        metrics: json!({
            "size_multiplier": multiplier.to_string(),
            "baseline_position_size_usd": baseline.avg_position_size_usd.to_string(),
            "post_loss_avg_size_usd": avg_size.to_string(),
            "post_loss_window": POST_LOSS_WINDOW,
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{TimeZone, Utc};

    use super::*;

    fn base_baseline(avg_size: Decimal) -> UserBaseline {
        UserBaseline {
            avg_trades_per_day: dec!(1),
            avg_position_size_usd: avg_size,
            typical_session_hours_utc: vec![13, 14, 15, 16],
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

    fn trade(hour: i64, pnl: Decimal, size: Decimal) -> TradeEvidence {
        let id = Uuid::new_v4();
        let opened = Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap()
            + chrono::Duration::hours(hour);
        let closed = opened + chrono::Duration::hours(1);
        TradeEvidence {
            id,
            short_id: id.simple().to_string().chars().take(8).collect(),
            symbol: "BTC_USDT".to_string(),
            side: "long".to_string(),
            opened_at: opened,
            closed_at: closed,
            pnl,
            r_multiple: None,
            setup_tag: None,
            position_size_usd: size,
        }
    }

    #[test]
    fn fires_when_post_loss_sizes_double_baseline() {
        // Baseline = $1000 avg size. Alternating loss → 2× size = $2000.
        let baseline = base_baseline(dec!(1000));
        let trades = vec![
            trade(0, dec!(-50), dec!(1000)),  // loss, triggers post-loss for next
            trade(1, dec!(-20), dec!(2000)),  // post-loss trade #1 ($2000)
            trade(2, dec!(-30), dec!(2000)),  // post-loss trade #2 ($2000)
            trade(3, dec!(10), dec!(2000)),   // post-loss trade #3 ($2000)
        ];

        let result = detect_sizing_drift(&baseline, &trades, &empty_week_stats())
            .expect("expected sizing drift to fire");

        assert_eq!(result.pattern, PatternKind::SizingDrift);
        assert_eq!(result.severity, Severity::Notable);
        assert_eq!(result.evidence.len(), 3);
        assert_eq!(result.evidence[0], trades[1].id);
        assert_eq!(result.evidence[2], trades[3].id);
        assert_eq!(
            result.metrics["size_multiplier"],
            serde_json::Value::String("2".to_string())
        );
    }

    #[test]
    fn does_not_fire_when_post_loss_sizes_match_baseline() {
        let baseline = base_baseline(dec!(1000));
        let trades = vec![
            trade(0, dec!(-50), dec!(1000)),
            trade(1, dec!(-20), dec!(1000)),
            trade(2, dec!(-30), dec!(1100)),
            trade(3, dec!(10), dec!(950)),
        ];

        assert!(detect_sizing_drift(&baseline, &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn escalates_severity_at_large_multiplier() {
        let baseline = base_baseline(dec!(1000));
        let trades = vec![
            trade(0, dec!(-50), dec!(1000)),
            trade(1, dec!(-20), dec!(3000)),
            trade(2, dec!(-30), dec!(3000)),
            trade(3, dec!(10), dec!(3000)),
        ];

        let result = detect_sizing_drift(&baseline, &trades, &empty_week_stats())
            .expect("expected sizing drift to fire");

        assert_eq!(result.severity, Severity::Concerning);
    }

    #[test]
    fn requires_three_post_loss_trades() {
        let baseline = base_baseline(dec!(1000));
        let trades = vec![
            trade(0, dec!(-50), dec!(1000)),
            trade(1, dec!(-20), dec!(3000)),
            // Only 1 post-loss trade — not enough.
        ];

        assert!(detect_sizing_drift(&baseline, &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn returns_none_when_baseline_is_zero() {
        let baseline = base_baseline(Decimal::ZERO);
        let trades = vec![
            trade(0, dec!(-50), dec!(1000)),
            trade(1, dec!(-20), dec!(2000)),
            trade(2, dec!(-30), dec!(2000)),
            trade(3, dec!(10), dec!(2000)),
        ];

        assert!(detect_sizing_drift(&baseline, &trades, &empty_week_stats()).is_none());
    }
}
