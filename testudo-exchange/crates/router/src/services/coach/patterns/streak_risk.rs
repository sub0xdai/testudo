//! Streak-risk detector.
//!
//! Flags two distinct dangerous streaks within the week:
//!
//! - **Loss streak**: `LOSS_STREAK_MIN` (3) or more consecutive losses.
//! - **Winning-and-pyramiding streak**: `WIN_STREAK_MIN` (5) or more
//!   consecutive wins where position size is monotonically non-decreasing
//!   across the streak (textbook over-confidence after a hot run).
//!
//! Severity:
//! - `Concerning` for the win streak with non-decreasing sizes — the spec
//!   treats pyramiding into wins as the higher-risk failure mode.
//! - `Notable` for a loss streak.
//!
//! When both streaks fire in the same week, the win streak is reported
//! (single-flag contract matches the other detectors).

use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;

use super::super::types::{
    FlaggedPattern, PatternKind, Severity, TradeEvidence, UserBaseline, WeekStats,
};

const LOSS_STREAK_MIN: usize = 3;
const WIN_STREAK_MIN: usize = 5;

pub fn detect_streak_risk(
    _baseline: &UserBaseline,
    trades: &[TradeEvidence],
    _stats: &WeekStats,
) -> Option<FlaggedPattern> {
    if trades.is_empty() {
        return None;
    }

    let mut sorted: Vec<&TradeEvidence> = trades.iter().collect();
    sorted.sort_by_key(|t| t.opened_at);

    if let Some(flag) = longest_pyramid_win_streak(&sorted) {
        return Some(flag);
    }
    longest_loss_streak(&sorted)
}

fn longest_loss_streak(sorted: &[&TradeEvidence]) -> Option<FlaggedPattern> {
    let mut best: Vec<&TradeEvidence> = Vec::new();
    let mut current: Vec<&TradeEvidence> = Vec::new();

    for t in sorted {
        if t.pnl < Decimal::ZERO {
            current.push(*t);
            if current.len() > best.len() {
                best = current.clone();
            }
        } else {
            current.clear();
        }
    }

    if best.len() < LOSS_STREAK_MIN {
        return None;
    }

    let evidence: Vec<Uuid> = best.iter().map(|t| t.id).collect();
    let total_pnl: Decimal = best.iter().map(|t| t.pnl).sum();

    Some(FlaggedPattern {
        pattern: PatternKind::StreakRisk,
        severity: Severity::Notable,
        evidence,
        metrics: json!({
            "streak_kind": "loss",
            "streak_length": best.len(),
            "total_pnl": total_pnl.to_string(),
        }),
    })
}

fn longest_pyramid_win_streak(sorted: &[&TradeEvidence]) -> Option<FlaggedPattern> {
    let mut best: Vec<&TradeEvidence> = Vec::new();
    let mut current: Vec<&TradeEvidence> = Vec::new();

    for t in sorted {
        if t.pnl > Decimal::ZERO {
            let monotonic = current
                .last()
                .is_none_or(|prev| t.position_size_usd >= prev.position_size_usd);
            if monotonic {
                current.push(*t);
            } else {
                // Streak continues as a win run but pyramiding broken — restart.
                current.clear();
                current.push(*t);
            }
            if current.len() > best.len() {
                best = current.clone();
            }
        } else {
            current.clear();
        }
    }

    if best.len() < WIN_STREAK_MIN {
        return None;
    }

    let evidence: Vec<Uuid> = best.iter().map(|t| t.id).collect();
    let starting_size = best.first().map(|t| t.position_size_usd).unwrap_or_default();
    let ending_size = best.last().map(|t| t.position_size_usd).unwrap_or_default();
    let size_growth = if starting_size > Decimal::ZERO {
        ending_size / starting_size
    } else {
        Decimal::ZERO
    };

    Some(FlaggedPattern {
        pattern: PatternKind::StreakRisk,
        severity: Severity::Concerning,
        evidence,
        metrics: json!({
            "streak_kind": "win_pyramid",
            "streak_length": best.len(),
            "starting_size_usd": starting_size.to_string(),
            "ending_size_usd": ending_size.to_string(),
            "size_growth_multiplier": size_growth.to_string(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{TimeZone, Utc};
    use rust_decimal_macros::dec;

    use super::*;

    fn baseline() -> UserBaseline {
        UserBaseline {
            avg_trades_per_day: dec!(1),
            avg_position_size_usd: dec!(1000),
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
    fn fires_on_three_consecutive_losses() {
        let trades = vec![
            trade(0, dec!(-10), dec!(1000)),
            trade(1, dec!(-20), dec!(1000)),
            trade(2, dec!(-30), dec!(1000)),
        ];

        let result = detect_streak_risk(&baseline(), &trades, &empty_week_stats())
            .expect("expected streak risk to fire");

        assert_eq!(result.pattern, PatternKind::StreakRisk);
        assert_eq!(result.severity, Severity::Notable);
        assert_eq!(result.evidence.len(), 3);
        assert_eq!(result.metrics["streak_kind"], "loss");
        assert_eq!(result.metrics["streak_length"], 3);
    }

    #[test]
    fn fires_on_five_pyramiding_wins() {
        let trades = vec![
            trade(0, dec!(10), dec!(1000)),
            trade(1, dec!(15), dec!(1200)),
            trade(2, dec!(20), dec!(1500)),
            trade(3, dec!(25), dec!(1800)),
            trade(4, dec!(30), dec!(2000)),
        ];

        let result = detect_streak_risk(&baseline(), &trades, &empty_week_stats())
            .expect("expected streak risk to fire");

        assert_eq!(result.pattern, PatternKind::StreakRisk);
        assert_eq!(result.severity, Severity::Concerning);
        assert_eq!(result.evidence.len(), 5);
        assert_eq!(result.metrics["streak_kind"], "win_pyramid");
        assert_eq!(result.metrics["starting_size_usd"], "1000");
        assert_eq!(result.metrics["ending_size_usd"], "2000");
    }

    #[test]
    fn does_not_fire_on_mixed_results() {
        let trades = vec![
            trade(0, dec!(10), dec!(1000)),
            trade(1, dec!(-15), dec!(1000)),
            trade(2, dec!(20), dec!(1000)),
            trade(3, dec!(-5), dec!(1000)),
            trade(4, dec!(15), dec!(1000)),
        ];

        assert!(detect_streak_risk(&baseline(), &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn five_wins_with_decreasing_sizes_does_not_fire_pyramid() {
        // A pure win streak with shrinking sizes is not pyramiding — no flag,
        // even though the win run is long enough.
        let trades = vec![
            trade(0, dec!(10), dec!(2000)),
            trade(1, dec!(15), dec!(1800)),
            trade(2, dec!(20), dec!(1500)),
            trade(3, dec!(25), dec!(1200)),
            trade(4, dec!(30), dec!(1000)),
        ];

        assert!(detect_streak_risk(&baseline(), &trades, &empty_week_stats()).is_none());
    }

    #[test]
    fn win_streak_with_constant_size_counts_as_pyramid() {
        // Monotonic non-decreasing includes flat sizes — still a streak that
        // signals the trader did not pull back after a hot run.
        let trades = vec![
            trade(0, dec!(10), dec!(1000)),
            trade(1, dec!(15), dec!(1000)),
            trade(2, dec!(20), dec!(1000)),
            trade(3, dec!(25), dec!(1000)),
            trade(4, dec!(30), dec!(1000)),
        ];

        let result = detect_streak_risk(&baseline(), &trades, &empty_week_stats())
            .expect("expected pyramid streak with flat sizes");

        assert_eq!(result.severity, Severity::Concerning);
        assert_eq!(result.evidence.len(), 5);
    }

    #[test]
    fn win_streak_resets_on_size_drop_then_recovers() {
        // Wins 0..2 grow, win 3 drops, wins 3..7 grow again. The pyramiding
        // streak must restart at win 3 and reach length 5 to fire.
        let trades = vec![
            trade(0, dec!(10), dec!(1000)),
            trade(1, dec!(15), dec!(1200)),
            trade(2, dec!(20), dec!(1500)),
            // Pyramiding broken here; restart.
            trade(3, dec!(10), dec!(800)),
            trade(4, dec!(15), dec!(1000)),
            trade(5, dec!(20), dec!(1100)),
            trade(6, dec!(25), dec!(1200)),
            trade(7, dec!(30), dec!(1300)),
        ];

        let result = detect_streak_risk(&baseline(), &trades, &empty_week_stats())
            .expect("expected restarted pyramid to fire");

        assert_eq!(result.severity, Severity::Concerning);
        assert_eq!(result.evidence.len(), 5);
        assert_eq!(result.evidence[0], trades[3].id);
        assert_eq!(result.evidence[4], trades[7].id);
    }

    #[test]
    fn pyramid_win_streak_takes_precedence_over_loss_streak() {
        let trades = vec![
            // Loss streak of 3.
            trade(0, dec!(-10), dec!(1000)),
            trade(1, dec!(-20), dec!(1000)),
            trade(2, dec!(-30), dec!(1000)),
            // Pyramiding win streak of 5.
            trade(3, dec!(10), dec!(1000)),
            trade(4, dec!(15), dec!(1100)),
            trade(5, dec!(20), dec!(1200)),
            trade(6, dec!(25), dec!(1300)),
            trade(7, dec!(30), dec!(1400)),
        ];

        let result = detect_streak_risk(&baseline(), &trades, &empty_week_stats())
            .expect("expected pyramid to win");

        assert_eq!(result.severity, Severity::Concerning);
        assert_eq!(result.metrics["streak_kind"], "win_pyramid");
    }

    #[test]
    fn two_losses_does_not_fire() {
        let trades = vec![
            trade(0, dec!(-10), dec!(1000)),
            trade(1, dec!(-20), dec!(1000)),
        ];

        assert!(detect_streak_risk(&baseline(), &trades, &empty_week_stats()).is_none());
    }
}
