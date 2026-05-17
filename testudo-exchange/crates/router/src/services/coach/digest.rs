//! CoachDigest composer — baseline, week stats, and week trade evidence.
//!
//! T3 shipped the three SQL helpers feeding the detector orchestrator.
//! T4 composes them into `build_digest` with skip-rule gating:
//!   opt-out → lifetime-trades → week-trades → no-flags.
//! Each skip logs a structured `skip_reason` and returns `Ok(None)`.
//!
//! Conventions:
//! - All financial math in `rust_decimal::Decimal`.
//! - `win_rate` is a 0..1 fraction (LLM / UI multiplies by 100 for display).
//! - Week trades are bucketed by `opened_at`, not `closed_at` — pattern
//!   detectors care about behavior initiated this week, not trades that
//!   merely closed inside it.
//! - Baseline window is 30 days *ending at* `as_of` (exclusive upper bound).

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use super::patterns;
use super::types::{
    CoachConfig, CoachDigest, FlaggedPattern, SetupBaseline, TradeEvidence, UserBaseline, WeekStats,
};

/// Untagged-bucket label used for trades with NULL or empty `setup_tag`.
/// Matches RSK-02 `setup_breakdown` convention so coach baselines and the
/// analytics endpoint agree.
const UNTAGGED_LABEL: &str = "(untagged)";

// ── Pure helpers (unit-testable without a DB) ───────────────────────────────

/// Pick the top-N (UTC) hours by trade count from a hour→count list.
/// Ties broken by lower hour index for determinism.
pub(super) fn top_hours(hour_counts: &[(u8, i64)], limit: usize) -> Vec<u8> {
    let mut sorted: Vec<(u8, i64)> = hour_counts.to_vec();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted.into_iter().take(limit).map(|(h, _)| h).collect()
}

/// Populate a 24-slot UTC-hour histogram from grouped rows.
/// Out-of-range hours are silently dropped.
pub(super) fn build_hour_histogram(hour_counts: &[(u8, i64)]) -> [i64; 24] {
    let mut hist = [0i64; 24];
    for &(hour, ct) in hour_counts {
        if (hour as usize) < hist.len() {
            hist[hour as usize] = ct;
        }
    }
    hist
}

// ── SQL row types (private to this module) ──────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct AggRow {
    trade_count: i64,
    avg_position_size_usd: Decimal,
    win_rate: Decimal,
    avg_r_multiple: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
struct HourCountRow {
    hour: i32,
    ct: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct P90Row {
    p90: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
struct SetupRow {
    setup_tag: String,
    trade_count: i64,
    avg_r_multiple: Decimal,
    win_rate: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
struct WeekTotalsRow {
    trade_count: i64,
    win_rate: Decimal,
    total_pnl: Decimal,
    total_r: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
struct TradeEvidenceRow {
    id: Uuid,
    symbol: String,
    side: String,
    entry_price: Decimal,
    quantity: Decimal,
    opened_at: DateTime<Utc>,
    closed_at: DateTime<Utc>,
    net_pnl: Decimal,
    r_multiple: Option<Decimal>,
    setup_tag: Option<String>,
}

// ── Public async helpers ────────────────────────────────────────────────────

/// 30-day rolling baseline ending at `as_of` (exclusive upper bound on
/// `closed_at`). Returns zeroed counts when the user has no history in
/// the window — callers (T4) interpret that as "skip" via a separate
/// lifetime-trade-count check, not by inspecting the baseline.
pub async fn compute_user_baseline(
    analytics_pool: &PgPool,
    user_id: Uuid,
    as_of: DateTime<Utc>,
) -> Result<UserBaseline, sqlx::Error> {
    let window_start = as_of - chrono::Duration::days(30);

    // 1. Aggregate counts + averages over the 30-day window.
    let agg = sqlx::query_as::<_, AggRow>(
        "SELECT \
            COUNT(*)::BIGINT AS trade_count, \
            COALESCE(AVG(entry_price * quantity), 0)::NUMERIC AS avg_position_size_usd, \
            COALESCE( \
                (COUNT(*) FILTER (WHERE net_pnl > 0))::NUMERIC \
                / GREATEST(COUNT(*), 1), 0 \
            )::NUMERIC AS win_rate, \
            COALESCE(AVG(r_multiple) FILTER (WHERE r_multiple IS NOT NULL), 0)::NUMERIC \
                AS avg_r_multiple \
        FROM journal_trades \
        WHERE user_id = $1 AND closed_at >= $2 AND closed_at < $3",
    )
    .bind(user_id)
    .bind(window_start)
    .bind(as_of)
    .fetch_one(analytics_pool)
    .await?;

    // 2. Hour-of-day histogram for typical-session detection.
    let hour_rows = sqlx::query_as::<_, HourCountRow>(
        "SELECT \
            EXTRACT(HOUR FROM opened_at AT TIME ZONE 'UTC')::INT AS hour, \
            COUNT(*)::BIGINT AS ct \
        FROM journal_trades \
        WHERE user_id = $1 AND closed_at >= $2 AND closed_at < $3 \
        GROUP BY hour",
    )
    .bind(user_id)
    .bind(window_start)
    .bind(as_of)
    .fetch_all(analytics_pool)
    .await?;
    let hour_counts: Vec<(u8, i64)> = hour_rows
        .into_iter()
        .filter_map(|r| {
            if (0..24).contains(&r.hour) {
                Some((r.hour as u8, r.ct))
            } else {
                None
            }
        })
        .collect();
    let typical_session_hours_utc = top_hours(&hour_counts, 4);

    // 3. p90 trades-per-6h-window — only counts non-empty windows. An
    //    empty window contributes nothing to "what counts as busy" for
    //    this user's baseline.
    let p90 = sqlx::query_as::<_, P90Row>(
        "WITH bucketed AS ( \
            SELECT \
                FLOOR(EXTRACT(EPOCH FROM opened_at) / 21600)::BIGINT AS bucket, \
                COUNT(*)::BIGINT AS ct \
            FROM journal_trades \
            WHERE user_id = $1 AND closed_at >= $2 AND closed_at < $3 \
            GROUP BY bucket \
        ) \
        SELECT COALESCE( \
            percentile_cont(0.9) WITHIN GROUP (ORDER BY ct::NUMERIC), 0 \
        )::NUMERIC AS p90 \
        FROM bucketed",
    )
    .bind(user_id)
    .bind(window_start)
    .bind(as_of)
    .fetch_one(analytics_pool)
    .await?;

    // 4. Per-setup baselines (case-insensitive grouping; NULL/empty → "(untagged)").
    let setup_rows = sqlx::query_as::<_, SetupRow>(
        "SELECT \
            COALESCE(NULLIF(LOWER(setup_tag), ''), '(untagged)') AS setup_tag, \
            COUNT(*)::BIGINT AS trade_count, \
            COALESCE(AVG(r_multiple) FILTER (WHERE r_multiple IS NOT NULL), 0)::NUMERIC \
                AS avg_r_multiple, \
            COALESCE( \
                (COUNT(*) FILTER (WHERE net_pnl > 0))::NUMERIC \
                / GREATEST(COUNT(*), 1), 0 \
            )::NUMERIC AS win_rate \
        FROM journal_trades \
        WHERE user_id = $1 AND closed_at >= $2 AND closed_at < $3 \
        GROUP BY COALESCE(NULLIF(LOWER(setup_tag), ''), '(untagged)')",
    )
    .bind(user_id)
    .bind(window_start)
    .bind(as_of)
    .fetch_all(analytics_pool)
    .await?;

    let setup_baselines: HashMap<String, SetupBaseline> = setup_rows
        .into_iter()
        .map(|r| {
            (
                r.setup_tag,
                SetupBaseline {
                    trade_count: r.trade_count,
                    avg_r_multiple: r.avg_r_multiple,
                    win_rate: r.win_rate,
                },
            )
        })
        .collect();

    // avg_trades_per_day is a derived figure over the fixed 30-day window.
    let avg_trades_per_day = if agg.trade_count > 0 {
        Decimal::from(agg.trade_count) / Decimal::from(30)
    } else {
        Decimal::ZERO
    };

    Ok(UserBaseline {
        avg_trades_per_day,
        avg_position_size_usd: agg.avg_position_size_usd,
        typical_session_hours_utc,
        win_rate: agg.win_rate,
        avg_r_multiple: agg.avg_r_multiple,
        p90_trades_per_6h: p90.p90,
        setup_baselines,
    })
}

/// Aggregate stats for the analyzed week. `[week_start, week_end)` is the
/// half-open interval on `opened_at`.
pub async fn compute_week_stats(
    analytics_pool: &PgPool,
    user_id: Uuid,
    week_start: DateTime<Utc>,
    week_end: DateTime<Utc>,
) -> Result<WeekStats, sqlx::Error> {
    let totals = sqlx::query_as::<_, WeekTotalsRow>(
        "SELECT \
            COUNT(*)::BIGINT AS trade_count, \
            COALESCE( \
                (COUNT(*) FILTER (WHERE net_pnl > 0))::NUMERIC \
                / GREATEST(COUNT(*), 1), 0 \
            )::NUMERIC AS win_rate, \
            COALESCE(SUM(net_pnl), 0)::NUMERIC AS total_pnl, \
            COALESCE(SUM(r_multiple) FILTER (WHERE r_multiple IS NOT NULL), 0)::NUMERIC \
                AS total_r \
        FROM journal_trades \
        WHERE user_id = $1 AND opened_at >= $2 AND opened_at < $3",
    )
    .bind(user_id)
    .bind(week_start)
    .bind(week_end)
    .fetch_one(analytics_pool)
    .await?;

    let hour_rows = sqlx::query_as::<_, HourCountRow>(
        "SELECT \
            EXTRACT(HOUR FROM opened_at AT TIME ZONE 'UTC')::INT AS hour, \
            COUNT(*)::BIGINT AS ct \
        FROM journal_trades \
        WHERE user_id = $1 AND opened_at >= $2 AND opened_at < $3 \
        GROUP BY hour",
    )
    .bind(user_id)
    .bind(week_start)
    .bind(week_end)
    .fetch_all(analytics_pool)
    .await?;
    let hour_counts: Vec<(u8, i64)> = hour_rows
        .into_iter()
        .filter_map(|r| {
            if (0..24).contains(&r.hour) {
                Some((r.hour as u8, r.ct))
            } else {
                None
            }
        })
        .collect();
    let trades_by_hour_utc = build_hour_histogram(&hour_counts);

    let setup_rows = sqlx::query_as::<_, SetupRow>(
        "SELECT \
            COALESCE(NULLIF(LOWER(setup_tag), ''), '(untagged)') AS setup_tag, \
            COUNT(*)::BIGINT AS trade_count, \
            COALESCE(AVG(r_multiple) FILTER (WHERE r_multiple IS NOT NULL), 0)::NUMERIC \
                AS avg_r_multiple, \
            COALESCE( \
                (COUNT(*) FILTER (WHERE net_pnl > 0))::NUMERIC \
                / GREATEST(COUNT(*), 1), 0 \
            )::NUMERIC AS win_rate \
        FROM journal_trades \
        WHERE user_id = $1 AND opened_at >= $2 AND opened_at < $3 \
        GROUP BY COALESCE(NULLIF(LOWER(setup_tag), ''), '(untagged)')",
    )
    .bind(user_id)
    .bind(week_start)
    .bind(week_end)
    .fetch_all(analytics_pool)
    .await?;
    let by_setup: HashMap<String, SetupBaseline> = setup_rows
        .into_iter()
        .map(|r| {
            (
                r.setup_tag,
                SetupBaseline {
                    trade_count: r.trade_count,
                    avg_r_multiple: r.avg_r_multiple,
                    win_rate: r.win_rate,
                },
            )
        })
        .collect();

    Ok(WeekStats {
        trade_count: totals.trade_count,
        win_rate: totals.win_rate,
        total_pnl: totals.total_pnl,
        total_r: totals.total_r,
        trades_by_hour_utc,
        by_setup,
    })
}

/// Fetch the week's trades as `TradeEvidence`. `position_size_usd` is the
/// notional `entry_price * quantity` — RSK-02's `journal_trades` schema
/// does not persist a separate notional column. Bucketed by `opened_at`.
pub async fn fetch_week_trades(
    analytics_pool: &PgPool,
    user_id: Uuid,
    week_start: DateTime<Utc>,
    week_end: DateTime<Utc>,
) -> Result<Vec<TradeEvidence>, sqlx::Error> {
    let rows = sqlx::query_as::<_, TradeEvidenceRow>(
        "SELECT id, symbol, side, entry_price, quantity, opened_at, closed_at, \
            net_pnl, r_multiple, setup_tag \
        FROM journal_trades \
        WHERE user_id = $1 AND opened_at >= $2 AND opened_at < $3 \
        ORDER BY opened_at ASC",
    )
    .bind(user_id)
    .bind(week_start)
    .bind(week_end)
    .fetch_all(analytics_pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let short_id = format!("{}", r.id.simple());
            // First 8 hex chars — uuid::Simple is 32 hex chars.
            let short_id = short_id.chars().take(8).collect::<String>();
            // Normalize NULL/empty setup_tag to None so the downstream digest
            // does not have to special-case both shapes.
            let setup_tag = r
                .setup_tag
                .and_then(|s| if s.trim().is_empty() { None } else { Some(s) });
            TradeEvidence {
                id: r.id,
                short_id,
                symbol: r.symbol,
                side: r.side,
                opened_at: r.opened_at,
                closed_at: r.closed_at,
                pnl: r.net_pnl,
                r_multiple: r.r_multiple,
                setup_tag,
                position_size_usd: r.entry_price * r.quantity,
            }
        })
        .collect())
}

/// Compose a `CoachDigest` for `[week_start, week_end)` if the user qualifies.
///
/// Skip rules (each logs a `skip_reason` and returns `Ok(None)`):
/// 1. `users.coach_enabled = FALSE` → `"opt_out"`.
/// 2. Lifetime trade count < `config.min_lifetime_trades` → `"lifetime_below_threshold"`.
/// 3. Week trade count < `config.min_week_trades` → `"week_below_threshold"`.
/// 4. No pattern flagged by `patterns::detect_all` → `"no_flags"`.
///
/// Returns `Ok(Some(digest))` only when all four gates pass. `digest.flagged_trades`
/// is filtered to the union of all flagged patterns' `evidence` IDs so the
/// narrator only receives trades it can actually cite.
pub async fn build_digest(
    pool: &PgPool,
    analytics_pool: &PgPool,
    user_id: Uuid,
    week_start: DateTime<Utc>,
    week_end: DateTime<Utc>,
    config: &CoachConfig,
) -> Result<Option<CoachDigest>, sqlx::Error> {
    // 1. Opt-out check — per-user `users.coach_enabled` column.
    let pref: (bool,) = sqlx::query_as("SELECT coach_enabled FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    if !pref.0 {
        tracing::info!(
            user_id = %user_id,
            skip_reason = "opt_out",
            "coach: skipping digest",
        );
        return Ok(None);
    }

    // 2. Lifetime-trade threshold — cold-start guard.
    let lifetime: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::BIGINT FROM journal_trades WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(analytics_pool)
            .await?;
    if lifetime.0 < config.min_lifetime_trades {
        tracing::info!(
            user_id = %user_id,
            lifetime_trades = lifetime.0,
            required = config.min_lifetime_trades,
            skip_reason = "lifetime_below_threshold",
            "coach: skipping digest",
        );
        return Ok(None);
    }

    // 3. Parallel fan-out: baseline / week stats / week trades. Any DB error
    //    aborts the whole digest — a partial digest would mis-ground the LLM.
    let (baseline, week_stats, week_trades) = tokio::try_join!(
        compute_user_baseline(analytics_pool, user_id, week_end),
        compute_week_stats(analytics_pool, user_id, week_start, week_end),
        fetch_week_trades(analytics_pool, user_id, week_start, week_end),
    )?;

    Ok(compose_digest(
        user_id,
        week_start,
        week_end,
        config,
        baseline,
        week_stats,
        week_trades,
    ))
}

/// Pure composition step — gates on week-trade count + at-least-one flag, then
/// filters evidence trades. Extracted so tests can exercise skip rules and
/// `flagged_trades` filtering without touching a database.
pub(super) fn compose_digest(
    user_id: Uuid,
    week_start: DateTime<Utc>,
    week_end: DateTime<Utc>,
    config: &CoachConfig,
    baseline: UserBaseline,
    week_stats: WeekStats,
    week_trades: Vec<TradeEvidence>,
) -> Option<CoachDigest> {
    if week_stats.trade_count < config.min_week_trades {
        tracing::info!(
            user_id = %user_id,
            week_trades = week_stats.trade_count,
            required = config.min_week_trades,
            skip_reason = "week_below_threshold",
            "coach: skipping digest",
        );
        return None;
    }

    let flagged_patterns = patterns::detect_all(&baseline, &week_trades, &week_stats);
    if flagged_patterns.is_empty() {
        tracing::info!(
            user_id = %user_id,
            skip_reason = "no_flags",
            "coach: skipping digest",
        );
        return None;
    }

    let flagged_trades = collect_flagged_trades(&flagged_patterns, &week_trades);

    Some(CoachDigest {
        user_id,
        week_start,
        week_end,
        baseline,
        week_stats,
        flagged_patterns,
        flagged_trades,
    })
}

/// Filter `trades` down to the union of all flagged patterns' evidence IDs,
/// preserving the input ordering (chronological from `fetch_week_trades`).
fn collect_flagged_trades(
    flags: &[FlaggedPattern],
    trades: &[TradeEvidence],
) -> Vec<TradeEvidence> {
    let referenced: HashSet<Uuid> = flags
        .iter()
        .flat_map(|f| f.evidence.iter().copied())
        .collect();
    trades
        .iter()
        .filter(|t| referenced.contains(&t.id))
        .cloned()
        .collect()
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_hours_picks_highest_counts_with_lower_index_tiebreak() {
        let counts = vec![
            (3u8, 5i64),
            (14, 12),
            (15, 12),
            (9, 1),
            (22, 8),
            (0, 12),
        ];
        // Three hours tied at 12: 0, 14, 15 — sorted by lower hour first.
        // Then 22 (8). Top-4 = [0, 14, 15, 22].
        assert_eq!(top_hours(&counts, 4), vec![0, 14, 15, 22]);
    }

    #[test]
    fn top_hours_returns_empty_for_no_data() {
        let empty: Vec<(u8, i64)> = vec![];
        assert!(top_hours(&empty, 4).is_empty());
    }

    #[test]
    fn top_hours_truncates_when_fewer_than_limit() {
        let counts = vec![(13u8, 7i64), (14, 3)];
        assert_eq!(top_hours(&counts, 4), vec![13, 14]);
    }

    #[test]
    fn build_hour_histogram_populates_correct_slots() {
        let counts = vec![(0u8, 4i64), (13, 9), (23, 1)];
        let hist = build_hour_histogram(&counts);
        assert_eq!(hist[0], 4);
        assert_eq!(hist[13], 9);
        assert_eq!(hist[23], 1);
        assert_eq!(hist[5], 0);
        assert_eq!(hist.iter().sum::<i64>(), 14);
    }

    #[test]
    fn build_hour_histogram_silently_drops_out_of_range() {
        let counts = vec![(24u8, 99i64), (10, 3)];
        let hist = build_hour_histogram(&counts);
        assert_eq!(hist[10], 3);
        assert_eq!(hist.iter().sum::<i64>(), 3);
    }

    #[test]
    fn untagged_label_matches_rsk02_convention() {
        // Coach baselines + RSK-02 `setup_breakdown` MUST agree on this label
        // so per-setup detection (T3d) and the analytics endpoint reference
        // the same bucket.
        assert_eq!(UNTAGGED_LABEL, "(untagged)");
    }

    // ── compose_digest / collect_flagged_trades fixtures ────────────────

    use chrono::TimeZone;
    use rust_decimal_macros::dec;

    use super::super::types::{
        CoachConfig, FlaggedPattern, PatternKind, Severity,
    };

    fn default_config() -> CoachConfig {
        CoachConfig {
            min_lifetime_trades: 30,
            min_week_trades: 3,
            enabled_global: true,
        }
    }

    fn baseline_with_avg_size(avg_size: Decimal) -> UserBaseline {
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

    fn week_stats_with_count(trade_count: i64) -> WeekStats {
        WeekStats {
            trade_count,
            win_rate: Decimal::ZERO,
            total_pnl: Decimal::ZERO,
            total_r: Decimal::ZERO,
            trades_by_hour_utc: [0; 24],
            by_setup: HashMap::new(),
        }
    }

    fn fixture_trade(hour: i64, pnl: Decimal, size: Decimal) -> TradeEvidence {
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

    fn flag_for(trade_ids: Vec<Uuid>) -> FlaggedPattern {
        FlaggedPattern {
            pattern: PatternKind::SizingDrift,
            severity: Severity::Notable,
            evidence: trade_ids,
            metrics: serde_json::json!({}),
        }
    }

    #[test]
    fn collect_flagged_trades_includes_only_referenced_ids() {
        let trades = vec![
            fixture_trade(0, dec!(-10), dec!(1000)),
            fixture_trade(1, dec!(20), dec!(1000)),
            fixture_trade(2, dec!(-5), dec!(1000)),
        ];
        let flags = vec![flag_for(vec![trades[0].id, trades[2].id])];

        let filtered = collect_flagged_trades(&flags, &trades);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, trades[0].id);
        assert_eq!(filtered[1].id, trades[2].id);
    }

    #[test]
    fn collect_flagged_trades_deduplicates_across_flags() {
        // Two flags referencing the same trade id should still yield one evidence row.
        let trades = vec![
            fixture_trade(0, dec!(-10), dec!(1000)),
            fixture_trade(1, dec!(20), dec!(1000)),
        ];
        let flags = vec![
            flag_for(vec![trades[0].id]),
            flag_for(vec![trades[0].id, trades[1].id]),
        ];

        let filtered = collect_flagged_trades(&flags, &trades);
        assert_eq!(filtered.len(), 2);
        // Ordering follows input-trade order, not flag-evidence order.
        assert_eq!(filtered[0].id, trades[0].id);
        assert_eq!(filtered[1].id, trades[1].id);
    }

    #[test]
    fn compose_digest_skips_when_week_count_below_threshold() {
        let user_id = Uuid::new_v4();
        let week_start = Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap();
        let week_end = week_start + chrono::Duration::days(7);
        let config = default_config();
        let baseline = baseline_with_avg_size(dec!(1000));
        // min_week_trades = 3; trade_count = 2 → skip.
        let week_stats = week_stats_with_count(2);

        let result = compose_digest(
            user_id,
            week_start,
            week_end,
            &config,
            baseline,
            week_stats,
            vec![
                fixture_trade(0, dec!(-10), dec!(1000)),
                fixture_trade(1, dec!(20), dec!(1000)),
            ],
        );
        assert!(result.is_none());
    }

    #[test]
    fn compose_digest_skips_when_no_patterns_flagged() {
        // Three uneventful within-baseline trades — no detector fires.
        let user_id = Uuid::new_v4();
        let week_start = Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap();
        let week_end = week_start + chrono::Duration::days(7);
        let config = default_config();
        let baseline = baseline_with_avg_size(dec!(1000));
        let week_stats = week_stats_with_count(3);

        // All trades at typical hours (13, 14, 15), no losses, same size.
        let trades = vec![
            fixture_trade(13, dec!(5), dec!(1000)),
            fixture_trade(14, dec!(5), dec!(1000)),
            fixture_trade(15, dec!(5), dec!(1000)),
        ];

        let result = compose_digest(
            user_id,
            week_start,
            week_end,
            &config,
            baseline,
            week_stats,
            trades,
        );
        assert!(result.is_none());
    }

    #[test]
    fn compose_digest_returns_digest_with_filtered_evidence() {
        // Sizing drift fires: 3 post-loss trades at 2× baseline. An unrelated
        // late-week trade is present → `flagged_trades` must exclude it.
        //
        // Fixture design notes:
        //   - Baseline p90 trades-per-6h is raised to 10 so frequency_spike
        //     cannot fire on our 4 in-window trades and pull the bystander
        //     into its evidence.
        //   - The bystander is placed days after the trigger run so it sits
        //     in a separate 6h bucket + a separate loss-streak slot.
        let user_id = Uuid::new_v4();
        let week_start = Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap();
        let week_end = week_start + chrono::Duration::days(7);
        let config = default_config();
        let mut baseline = baseline_with_avg_size(dec!(1000));
        baseline.p90_trades_per_6h = dec!(10);
        let week_stats = week_stats_with_count(5);

        let loss = fixture_trade(13, dec!(-50), dec!(1000));
        let post_loss_1 = fixture_trade(14, dec!(-20), dec!(2000));
        let post_loss_2 = fixture_trade(15, dec!(-30), dec!(2000));
        let post_loss_3 = fixture_trade(16, dec!(10), dec!(2000));
        // Unrelated trade ~3 days later — separate 6h bucket, breaks streak.
        let bystander = fixture_trade(13 + 24 * 3, dec!(5), dec!(1000));
        let bystander_id = bystander.id;

        let trades = vec![loss, post_loss_1, post_loss_2, post_loss_3, bystander];

        let digest = compose_digest(
            user_id,
            week_start,
            week_end,
            &config,
            baseline,
            week_stats,
            trades,
        )
        .expect("expected digest when sizing_drift fires");

        assert_eq!(digest.user_id, user_id);
        assert_eq!(digest.week_start, week_start);
        assert_eq!(digest.week_end, week_end);
        assert!(!digest.flagged_patterns.is_empty());
        assert!(digest
            .flagged_patterns
            .iter()
            .any(|p| p.pattern == PatternKind::SizingDrift));
        // Bystander must not appear in filtered evidence.
        assert!(digest.flagged_trades.iter().all(|t| t.id != bystander_id));
    }

    #[test]
    fn compose_digest_produces_serializable_shape() {
        // Shape contract: a composed digest round-trips through JSON without
        // panicking and preserves the expected top-level keys — guards the
        // narrator wire contract against accidental type churn.
        let user_id = Uuid::new_v4();
        let week_start = Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap();
        let week_end = week_start + chrono::Duration::days(7);
        let config = default_config();
        let baseline = baseline_with_avg_size(dec!(1000));
        let week_stats = week_stats_with_count(4);

        let trades = vec![
            fixture_trade(13, dec!(-50), dec!(1000)),
            fixture_trade(14, dec!(-20), dec!(2000)),
            fixture_trade(15, dec!(-30), dec!(2000)),
            fixture_trade(16, dec!(10), dec!(2000)),
        ];

        let digest = compose_digest(
            user_id,
            week_start,
            week_end,
            &config,
            baseline,
            week_stats,
            trades,
        )
        .expect("digest should compose");

        let json = serde_json::to_value(&digest).expect("digest serializes");
        let obj = json.as_object().expect("digest is a JSON object");
        for key in [
            "user_id",
            "week_start",
            "week_end",
            "baseline",
            "week_stats",
            "flagged_patterns",
            "flagged_trades",
        ] {
            assert!(obj.contains_key(key), "digest missing key {key}");
        }
    }
}
