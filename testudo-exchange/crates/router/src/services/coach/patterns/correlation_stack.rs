//! Correlation-stack detector.
//!
//! Flags when the trader concentrates directional exposure in a single asset
//! family. Trades are grouped by `(bucket_for(extract_base_asset(symbol)),
//! side)` using the RSK-01 asset-family taxonomy. For each group we run a
//! sweep-line over open/close events and find the longest continuous window
//! during which at least `MIN_CONCURRENT` positions are simultaneously open.
//! The worst (bucket, side) group is picked by `(peak_concurrent, duration)`
//! and flagged if its window exceeds `MIN_DURATION_HOURS`.
//!
//! Severity:
//! - `Concerning` when `peak_concurrent >= 4` or `duration > 8h`
//! - `Notable` otherwise
//!
//! The `stables` bucket is excluded (base-asset stablecoins are rarely
//! directional positions and would be noise in the coach output).

// @anchor exchange:router:correlation_stack
// @tags api

use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;

use super::super::types::{
    FlaggedPattern, PatternKind, Severity, TradeEvidence, UserBaseline, WeekStats,
};
use crate::services::risk_snapshot::{bucket_for, extract_base_asset};

/// Minimum concurrent positions in a (bucket, side) to be considered a stack.
const MIN_CONCURRENT: usize = 3;
/// Stacks must persist strictly longer than this to flag.
const MIN_DURATION_HOURS: i64 = 4;
/// Duration beyond this escalates severity to `Concerning`.
const CONCERNING_DURATION_HOURS: i64 = 8;
/// Peak concurrent positions at or above this escalates severity to `Concerning`.
const CONCERNING_CONCURRENT: usize = 4;
/// Bucket excluded from correlation analysis (stablecoins aren't directional).
const EXCLUDED_BUCKET: &str = "stables";

pub fn detect_correlation_stack(
    _baseline: &UserBaseline,
    trades: &[TradeEvidence],
    _stats: &WeekStats,
) -> Option<FlaggedPattern> {
    if trades.len() < MIN_CONCURRENT {
        return None;
    }

    // Group trades by (bucket, normalized_side).
    let mut groups: BTreeMap<(&'static str, String), Vec<&TradeEvidence>> = BTreeMap::new();
    for t in trades {
        let bucket = bucket_for(&extract_base_asset(&t.symbol));
        if bucket == EXCLUDED_BUCKET {
            continue;
        }
        let side = t.side.to_lowercase();
        if side != "long" && side != "short" {
            continue;
        }
        groups.entry((bucket, side)).or_default().push(t);
    }

    let min_duration = Duration::hours(MIN_DURATION_HOURS);

    let mut worst: Option<(&'static str, String, usize, Duration, Vec<Uuid>)> = None;

    for ((bucket, side), group_trades) in groups {
        if group_trades.len() < MIN_CONCURRENT {
            continue;
        }
        let Some((peak, duration, evidence)) =
            max_concurrent_window(&group_trades, MIN_CONCURRENT, min_duration)
        else {
            continue;
        };

        let candidate = (bucket, side.clone(), peak, duration, evidence);
        match &worst {
            Some((_, _, best_peak, best_dur, _))
                if (*best_peak, *best_dur) >= (candidate.2, candidate.3) => {}
            _ => worst = Some(candidate),
        }
    }

    let (bucket, side, peak, duration, evidence) = worst?;

    let severity = if peak >= CONCERNING_CONCURRENT
        || duration > Duration::hours(CONCERNING_DURATION_HOURS)
    {
        Severity::Concerning
    } else {
        Severity::Notable
    };

    let duration_hours = Decimal::from(duration.num_minutes()) / Decimal::from(60i64);

    Some(FlaggedPattern {
        pattern: PatternKind::CorrelationStack,
        severity,
        evidence,
        metrics: json!({
            "bucket": bucket,
            "side": side,
            "peak_concurrent": peak,
            "duration_hours": duration_hours.to_string(),
            "min_duration_hours": MIN_DURATION_HOURS,
            "min_concurrent": MIN_CONCURRENT,
        }),
    })
}

/// Sweep-line over open/close events. Returns `(peak_concurrent, duration,
/// evidence_trade_ids)` for the longest continuous run where concurrent open
/// positions ≥ `min_concurrent` and the run exceeds `min_duration`. Evidence
/// is the set of trades active at any point during that run, sorted by id for
/// deterministic output.
fn max_concurrent_window(
    trades: &[&TradeEvidence],
    min_concurrent: usize,
    min_duration: Duration,
) -> Option<(usize, Duration, Vec<Uuid>)> {
    #[derive(Clone, Copy)]
    struct Event {
        ts: DateTime<Utc>,
        delta: i8,
        id: Uuid,
    }

    let mut events: Vec<Event> = Vec::with_capacity(trades.len() * 2);
    for t in trades {
        events.push(Event {
            ts: t.opened_at,
            delta: 1,
            id: t.id,
        });
        events.push(Event {
            ts: t.closed_at,
            delta: -1,
            id: t.id,
        });
    }
    // Close (-1) before open (+1) on the same timestamp so back-to-back trades
    // don't spuriously inflate the concurrent count.
    events.sort_by(|a, b| a.ts.cmp(&b.ts).then_with(|| a.delta.cmp(&b.delta)));

    let mut active: HashSet<Uuid> = HashSet::new();
    let mut count: i32 = 0;
    let mut run_start: Option<DateTime<Utc>> = None;
    let mut run_active: HashSet<Uuid> = HashSet::new();
    let mut run_peak: usize = 0;
    let mut best: Option<(usize, Duration, HashSet<Uuid>)> = None;

    for ev in &events {
        if ev.delta > 0 {
            active.insert(ev.id);
            count += 1;
        } else {
            active.remove(&ev.id);
            count -= 1;
        }

        let threshold_met = count >= min_concurrent as i32;

        if threshold_met {
            if run_start.is_none() {
                run_start = Some(ev.ts);
                run_active.clear();
                run_peak = 0;
            }
            run_active.extend(active.iter().copied());
            if (count as usize) > run_peak {
                run_peak = count as usize;
            }
        } else if let Some(start) = run_start.take() {
            let duration = ev.ts - start;
            if duration > min_duration {
                let better = match &best {
                    None => true,
                    Some((peak, dur, _)) => (run_peak, duration) > (*peak, *dur),
                };
                if better {
                    best = Some((run_peak, duration, run_active.clone()));
                }
            }
            run_active.clear();
            run_peak = 0;
        }
    }

    best.map(|(peak, dur, ids)| {
        let mut v: Vec<Uuid> = ids.into_iter().collect();
        v.sort();
        (peak, dur, v)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{Duration as ChronoDuration, TimeZone, Utc};
    use rust_decimal_macros::dec;

    use super::*;

    fn empty_baseline() -> UserBaseline {
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

    fn trade(
        symbol: &str,
        side: &str,
        open_hour: i64,
        close_hour: i64,
    ) -> TradeEvidence {
        let id = Uuid::new_v4();
        let base = Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap();
        TradeEvidence {
            id,
            short_id: id.simple().to_string().chars().take(8).collect(),
            symbol: symbol.to_string(),
            side: side.to_string(),
            opened_at: base + ChronoDuration::hours(open_hour),
            closed_at: base + ChronoDuration::hours(close_hour),
            pnl: dec!(0),
            r_multiple: Some(dec!(0)),
            setup_tag: None,
            position_size_usd: dec!(1000),
        }
    }

    #[test]
    fn fires_on_three_concurrent_longs_in_l2_bucket_for_six_hours() {
        // ARB, OP, MATIC all in L2 bucket, all long, all open from hour 0..=6.
        // Concurrent window = 6h > 4h threshold.
        let trades = vec![
            trade("ARB/USDT", "long", 0, 6),
            trade("OP/USDT", "long", 0, 6),
            trade("MATIC/USDT", "long", 0, 6),
        ];

        let result = detect_correlation_stack(&empty_baseline(), &trades, &empty_week_stats())
            .expect("expected correlation stack to fire");

        assert_eq!(result.pattern, PatternKind::CorrelationStack);
        assert_eq!(result.severity, Severity::Notable);
        assert_eq!(result.evidence.len(), 3);
        assert_eq!(result.metrics["bucket"], "L2");
        assert_eq!(result.metrics["side"], "long");
        assert_eq!(result.metrics["peak_concurrent"], 3);
    }

    #[test]
    fn does_not_fire_when_positions_are_sequential_not_concurrent() {
        // Three ETH-beta longs but each closes before the next opens.
        let trades = vec![
            trade("ETH/USDT", "long", 0, 2),
            trade("WETH/USDT", "long", 3, 5),
            trade("STETH/USDT", "long", 6, 8),
        ];

        assert!(
            detect_correlation_stack(&empty_baseline(), &trades, &empty_week_stats()).is_none()
        );
    }

    #[test]
    fn does_not_fire_across_different_buckets() {
        // BTC + ETH + SOL are in three different buckets — no stack.
        let trades = vec![
            trade("BTC/USDT", "long", 0, 10),
            trade("ETH/USDT", "long", 0, 10),
            trade("SOL/USDT", "long", 0, 10),
        ];

        assert!(
            detect_correlation_stack(&empty_baseline(), &trades, &empty_week_stats()).is_none()
        );
    }

    #[test]
    fn does_not_fire_when_sides_differ() {
        // Same bucket (L2) but mixed directions — not a one-way stack.
        let trades = vec![
            trade("ARB/USDT", "long", 0, 10),
            trade("OP/USDT", "short", 0, 10),
            trade("MATIC/USDT", "long", 0, 10),
        ];

        assert!(
            detect_correlation_stack(&empty_baseline(), &trades, &empty_week_stats()).is_none()
        );
    }

    #[test]
    fn does_not_fire_below_duration_threshold() {
        // 3 concurrent L2 longs but only for 3h (< 4h minimum).
        let trades = vec![
            trade("ARB/USDT", "long", 0, 3),
            trade("OP/USDT", "long", 0, 3),
            trade("MATIC/USDT", "long", 0, 3),
        ];

        assert!(
            detect_correlation_stack(&empty_baseline(), &trades, &empty_week_stats()).is_none()
        );
    }

    #[test]
    fn escalates_severity_when_four_concurrent() {
        // Four concurrent alt-L1 longs → Concerning.
        let trades = vec![
            trade("SOL/USDT", "long", 0, 6),
            trade("AVAX/USDT", "long", 0, 6),
            trade("NEAR/USDT", "long", 0, 6),
            trade("DOT/USDT", "long", 0, 6),
        ];

        let result = detect_correlation_stack(&empty_baseline(), &trades, &empty_week_stats())
            .expect("expected correlation stack to fire");

        assert_eq!(result.severity, Severity::Concerning);
        assert_eq!(result.metrics["peak_concurrent"], 4);
        assert_eq!(result.evidence.len(), 4);
    }

    #[test]
    fn escalates_severity_when_duration_exceeds_eight_hours() {
        // Exactly 3 concurrent L2 longs held for 10h → Concerning.
        let trades = vec![
            trade("ARB/USDT", "long", 0, 10),
            trade("OP/USDT", "long", 0, 10),
            trade("STRK/USDT", "long", 0, 10),
        ];

        let result = detect_correlation_stack(&empty_baseline(), &trades, &empty_week_stats())
            .expect("expected correlation stack to fire");

        assert_eq!(result.severity, Severity::Concerning);
        assert_eq!(result.metrics["peak_concurrent"], 3);
    }

    #[test]
    fn picks_worst_group_when_multiple_buckets_qualify() {
        // L2 group: 3 concurrent for 5h (Notable).
        // alt-L1 group: 4 concurrent for 6h (Concerning, higher peak).
        let trades = vec![
            // L2 longs
            trade("ARB/USDT", "long", 0, 5),
            trade("OP/USDT", "long", 0, 5),
            trade("MATIC/USDT", "long", 0, 5),
            // alt-L1 longs — should win as the worst
            trade("SOL/USDT", "long", 0, 6),
            trade("AVAX/USDT", "long", 0, 6),
            trade("NEAR/USDT", "long", 0, 6),
            trade("DOT/USDT", "long", 0, 6),
        ];

        let result = detect_correlation_stack(&empty_baseline(), &trades, &empty_week_stats())
            .expect("expected correlation stack to fire");

        assert_eq!(result.metrics["bucket"], "alt-L1");
        assert_eq!(result.metrics["peak_concurrent"], 4);
        assert_eq!(result.severity, Severity::Concerning);
    }

    #[test]
    fn reuses_rsk01_extract_base_asset_for_varied_symbol_formats() {
        // Exercise BTC-beta bucket via WBTC symbol in mixed formats.
        let trades = vec![
            trade("BTC/USDT:USDT", "long", 0, 6),
            trade("WBTC-USDT", "long", 0, 6),
            trade("TBTC_USDT", "long", 0, 6),
        ];

        let result = detect_correlation_stack(&empty_baseline(), &trades, &empty_week_stats())
            .expect("expected correlation stack to fire");

        assert_eq!(result.metrics["bucket"], "BTC-beta");
    }
}
