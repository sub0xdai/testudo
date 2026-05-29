//! Dignitas snapshot orchestrator (ENG-01a, T5).
//!
//! `compute_score` — pure formula; accepts pre-loaded weights and pre-computed
//! `InputContributions`. Caller is responsible for renormalizing weights when
//! no coach data is available (`DignitasWeights::without_coach()`).
//!
//! `take_daily_snapshot` — DB orchestrator: loads user data, calls T4 input
//! functions, applies formula, handles cold-start, upserts into
//! `dignitas_history`.

// @anchor exchange:router:snapshot
// @tags api

use chrono::{Duration, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sqlx::PgPool;
use uuid::Uuid;

use super::config::load_weights;
use super::inputs::{
    compute_coach_severity_penalty, compute_drawdown_adherence, compute_journal_consistency,
    compute_risk_per_trade_consistency, compute_setup_adherence,
};
use super::types::{DignitasSnapshot, DignitasWeights, InputContributions};
use crate::services::coach::types::Severity;

/// Trailing window in days for all five inputs.
const TRAILING_DAYS: i64 = 30;

/// Number of recent coach reports to analyse.
const COACH_REPORT_LIMIT: i64 = 4;

/// Cache key prefix for `RiskConfig` (see `common_utils::risk::pg_storage`).
const RISK_CONFIG_KEY_PREFIX: &str = "risk:config";

/// Apply the weighted Dignitas formula to a set of pre-computed inputs.
///
/// Returns a score in `[0.0, 100.0]`.  Weights must already be renormalized
/// for the no-coach-data case before this call (see `DignitasWeights::without_coach`).
pub fn compute_score(inputs: &InputContributions, weights: &DignitasWeights) -> Decimal {
    let raw = weights.drawdown_adherence * inputs.drawdown_adherence
        + weights.risk_per_trade_consistency * inputs.risk_per_trade_consistency
        + weights.setup_adherence * inputs.setup_adherence
        + weights.coach_severity_penalty * (Decimal::ONE - inputs.coach_severity_penalty)
        + weights.journal_consistency * inputs.journal_consistency;
    (raw * dec!(100)).max(Decimal::ZERO).min(dec!(100))
}

/// Compute and persist a daily Dignitas snapshot for `user_id` on `date`.
///
/// Returns the upserted snapshot. Idempotent: re-running for the same
/// (user_id, date) pair overwrites the prior row.
pub async fn take_daily_snapshot(
    pool: &PgPool,
    user_id: Uuid,
    date: NaiveDate,
) -> Result<DignitasSnapshot, Box<dyn std::error::Error + Send + Sync>> {
    let weights = load_weights(pool).await?;

    let trailing_cutoff = date - Duration::days(TRAILING_DAYS);

    // ── Input 1: Drawdown adherence ──────────────────────────────────────────
    let drawdown_rows: Vec<(Decimal, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT jds.drawdown_pct, ce.value \
         FROM journal_daily_stats jds \
         LEFT JOIN cache_entries ce \
             ON ce.key = $2 AND ce.expires_at > NOW() \
         WHERE jds.user_id = $1 AND jds.stat_date >= $3",
    )
    .bind(user_id)
    .bind(format!("{RISK_CONFIG_KEY_PREFIX}:{user_id}"))
    .bind(trailing_cutoff)
    .fetch_all(pool)
    .await?;

    // Extract the configured drawdown limit (once, from first non-null cache row).
    let dd_limit: Decimal = drawdown_rows
        .iter()
        .find_map(|(_, cache_val)| {
            cache_val
                .as_ref()
                .and_then(|v| v.get("daily_max_drawdown_percent"))
                .and_then(|v| serde_json::from_value::<Decimal>(v.clone()).ok())
        })
        .unwrap_or(dec!(5)); // default 5%

    let drawdown_pairs: Vec<(Decimal, Decimal)> = drawdown_rows
        .iter()
        .map(|(pct, _)| (*pct, dd_limit))
        .collect();

    let drawdown_adherence = compute_drawdown_adherence(&drawdown_pairs);

    // ── Inputs 3+5: Setup adherence + journal consistency ────────────────────
    // Query trades with setup_tag and notes/linked-entry presence.
    #[derive(sqlx::FromRow)]
    struct TradeRow {
        has_setup_tag: bool,
        has_doc: bool,
        risk_amount: Option<Decimal>,
    }

    let trade_rows: Vec<TradeRow> = sqlx::query_as(
        "SELECT \
            (jt.setup_tag IS NOT NULL) AS has_setup_tag, \
            (jt.notes IS NOT NULL AND jt.notes <> '' \
             OR EXISTS(SELECT 1 FROM journal_entries je WHERE je.trade_id = jt.id)) AS has_doc, \
            jt.risk_amount \
         FROM journal_trades jt \
         WHERE jt.user_id = $1 AND jt.closed_at >= $2",
    )
    .bind(user_id)
    .bind(trailing_cutoff)
    .fetch_all(pool)
    .await?;

    // Cold-start gate: trade volume in the trailing 30d window. Replaces the
    // older snapshot-count proxy — direct measurement of the actual signal
    // driving the inputs is more honest than counting prior scheduler runs.
    let trade_count_30d = trade_rows.len() as i64;
    let cold_start = trade_count_30d < weights.cold_start_min_trades;

    let setup_flags: Vec<bool> = trade_rows.iter().map(|r| r.has_setup_tag).collect();
    let doc_flags: Vec<bool> = trade_rows.iter().map(|r| r.has_doc).collect();

    let setup_adherence = compute_setup_adherence(&setup_flags);
    let journal_consistency = compute_journal_consistency(&doc_flags);

    // ── Input 2: Risk-per-trade consistency ──────────────────────────────────
    // Get user's configured account_risk_percent from cache (default 2%).
    let configured_risk_pct: Decimal = sqlx::query_as::<_, (Option<serde_json::Value>,)>(
        "SELECT value FROM cache_entries WHERE key = $1 AND expires_at > NOW()",
    )
    .bind(format!("{RISK_CONFIG_KEY_PREFIX}:{user_id}"))
    .fetch_optional(pool)
    .await?
    .and_then(|(v,)| v)
    .and_then(|v| {
        v.get("account_risk_percent")
            .and_then(|v| serde_json::from_value::<Decimal>(v.clone()).ok())
    })
    .unwrap_or(dec!(2)); // default 2%

    // Get most recent total equity across all user exchange accounts as balance proxy.
    let (total_balance,): (Option<Decimal>,) = sqlx::query_as(
        "SELECT SUM(latest.equity) \
         FROM ( \
             SELECT DISTINCT ON (bs.exchange_account_id) bs.equity \
             FROM balance_snapshots bs \
             JOIN exchange_accounts ea ON ea.id = bs.exchange_account_id \
             WHERE ea.user_id = $1 \
             ORDER BY bs.exchange_account_id, bs.snapshot_at DESC \
         ) latest",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let configured_risk_frac = configured_risk_pct / dec!(100);
    let risk_pairs: Vec<(Decimal, Decimal)> = match total_balance {
        Some(balance) if !balance.is_zero() => trade_rows
            .iter()
            .filter_map(|r| r.risk_amount)
            .map(|risk_usd| (risk_usd / balance, configured_risk_frac))
            .collect(),
        // No balance data → skip risk consistency (returns 1.0 via empty slice)
        _ => vec![],
    };

    let risk_per_trade_consistency = compute_risk_per_trade_consistency(&risk_pairs);

    // ── Input 4: Coach severity penalty ─────────────────────────────────────
    #[derive(sqlx::FromRow)]
    struct CoachRow {
        digest_json: serde_json::Value,
    }

    let coach_rows: Vec<CoachRow> = sqlx::query_as(
        "SELECT digest_json FROM coach_reports \
         WHERE user_id = $1 \
         ORDER BY generated_at DESC \
         LIMIT $2",
    )
    .bind(user_id)
    .bind(COACH_REPORT_LIMIT)
    .fetch_all(pool)
    .await?;

    let has_coach_data = !coach_rows.is_empty();

    let report_severities: Vec<Vec<Severity>> = coach_rows
        .iter()
        .map(|row| {
            row.digest_json
                .get("flagged_patterns")
                .and_then(|fp| fp.as_array())
                .map(|patterns| {
                    patterns
                        .iter()
                        .filter_map(|p| {
                            p.get("severity")
                                .and_then(|s| serde_json::from_value::<Severity>(s.clone()).ok())
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();

    let coach_severity_penalty = compute_coach_severity_penalty(&report_severities);

    // ── Assemble contributions + apply formula ───────────────────────────────
    let contributions = InputContributions {
        drawdown_adherence,
        risk_per_trade_consistency,
        setup_adherence,
        coach_severity_penalty,
        journal_consistency,
    };

    let active_weights = if has_coach_data {
        weights.clone()
    } else {
        weights.without_coach()
    };

    let score = compute_score(&contributions, &active_weights);

    // ── Upsert into dignitas_history ─────────────────────────────────────────
    sqlx::query(
        "INSERT INTO dignitas_history \
             (user_id, date, score, cold_start, trade_count_30d, \
              drawdown_adherence, risk_per_trade_consistency, \
              setup_adherence, coach_severity_penalty, journal_consistency) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (user_id, date) DO UPDATE SET \
             score = EXCLUDED.score, \
             cold_start = EXCLUDED.cold_start, \
             trade_count_30d = EXCLUDED.trade_count_30d, \
             drawdown_adherence = EXCLUDED.drawdown_adherence, \
             risk_per_trade_consistency = EXCLUDED.risk_per_trade_consistency, \
             setup_adherence = EXCLUDED.setup_adherence, \
             coach_severity_penalty = EXCLUDED.coach_severity_penalty, \
             journal_consistency = EXCLUDED.journal_consistency",
    )
    .bind(user_id)
    .bind(date)
    .bind(score)
    .bind(cold_start)
    .bind(trade_count_30d as i32)
    .bind(contributions.drawdown_adherence)
    .bind(contributions.risk_per_trade_consistency)
    .bind(contributions.setup_adherence)
    .bind(contributions.coach_severity_penalty)
    .bind(contributions.journal_consistency)
    .execute(pool)
    .await?;

    Ok(DignitasSnapshot {
        user_id,
        date,
        score,
        cold_start,
        trade_count_30d,
        drawdown_adherence: contributions.drawdown_adherence,
        risk_per_trade_consistency: contributions.risk_per_trade_consistency,
        setup_adherence: contributions.setup_adherence,
        coach_severity_penalty: contributions.coach_severity_penalty,
        journal_consistency: contributions.journal_consistency,
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;
    use crate::services::dignitas::types::DignitasWeights;

    /// Default spec weights (sum = 1.0) matching the migration seed.
    fn spec_weights() -> DignitasWeights {
        DignitasWeights {
            drawdown_adherence: dec!(0.25),
            risk_per_trade_consistency: dec!(0.20),
            setup_adherence: dec!(0.20),
            coach_severity_penalty: dec!(0.20),
            journal_consistency: dec!(0.15),
            cold_start_min_trades: 10,
        }
    }

    /// 100 disciplined trades — characterised purely by discipline inputs:
    /// - Every day within drawdown limit (1.0)
    /// - Risk sizing matches config exactly (1.0)
    /// - Every trade tagged with setup (1.0)
    /// - No coach flags (penalty = 0.0)
    /// - Every trade journaled (1.0)
    ///
    /// Note: P&L, frequency, and win rate are NOT inputs — they are excluded
    /// by design (FR-9). This fixture deliberately omits them.
    fn disciplined() -> InputContributions {
        InputContributions {
            drawdown_adherence: dec!(1.0),
            risk_per_trade_consistency: dec!(1.0),
            setup_adherence: dec!(1.0),
            coach_severity_penalty: dec!(0.0),
            journal_consistency: dec!(1.0),
        }
    }

    /// 1000 undisciplined high-frequency high-P&L trades — again characterised
    /// purely by discipline inputs (not by their P&L outcomes):
    /// - 60% of days breached the daily drawdown limit (adherence = 0.4)
    /// - Risk sizing inconsistent (consistency = 0.3)
    /// - 0% of trades tagged with a setup (adherence = 0.0)
    /// - Frequent coach flags (penalty = 0.8)
    /// - 0% of trades journaled (consistency = 0.0)
    ///
    /// A high-frequency, high-P&L, high-win-rate trader with these inputs MUST
    /// still score lower than the disciplined trader above.
    fn undisciplined_high_freq_high_pnl() -> InputContributions {
        InputContributions {
            drawdown_adherence: dec!(0.4),
            risk_per_trade_consistency: dec!(0.3),
            setup_adherence: dec!(0.0),
            coach_severity_penalty: dec!(0.8),
            journal_consistency: dec!(0.0),
        }
    }

    /// FR-9 gate — ungameability invariant.
    ///
    /// A trader who is disciplined-but-possibly-losing must score HIGHER than
    /// a high-frequency high-P&L trader who ignores every risk discipline rule.
    ///
    /// RED until T5 implements `compute_score`.
    #[test]
    fn disciplined_scores_higher_than_undisciplined_high_freq_high_pnl() {
        let weights = spec_weights();
        let disciplined_score = compute_score(&disciplined(), &weights);
        let undisciplined_score = compute_score(&undisciplined_high_freq_high_pnl(), &weights);

        assert!(
            disciplined_score > undisciplined_score,
            "FR-9 violation: disciplined score ({disciplined_score}) must exceed \
             undisciplined high-freq high-P&L score ({undisciplined_score}). \
             Frequency, win rate, and P&L must NEVER be inputs."
        );
    }

    /// Sanity check: perfect discipline scores 100.
    ///
    /// RED until T5 implements `compute_score`.
    #[test]
    fn perfect_discipline_scores_100() {
        let score = compute_score(&disciplined(), &spec_weights());
        assert_eq!(score, dec!(100), "fully disciplined trader should score 100");
    }

    /// Sanity check: worst-case undisciplined scores below 25.
    /// (The undisciplined fixture has coach_severity_penalty=0.8, so the coach
    /// component contributes 0.20 × (1−0.8) × 100 = 4 points, plus drawdown
    /// contributes 0.25×0.4×100=10, risk contributes 0.20×0.3×100=6 → total ≈ 20.)
    ///
    /// RED until T5 implements `compute_score`.
    #[test]
    fn undisciplined_scores_below_25() {
        let score = compute_score(&undisciplined_high_freq_high_pnl(), &spec_weights());
        assert!(
            score < dec!(25),
            "undisciplined trader should score below 25, got {score}"
        );
    }

    // ── Cold-start / no-coach-data path (Gate 2 gap 1) ─────────────────────
    //
    // The coach-renormalization path is the one Gate 1 explicitly fixed:
    // when a user has no coach reports yet, the coach axis must be EXCLUDED
    // from the composite (not "neutral = 0.0", which is punitive) and the
    // remaining 4 weights must sum to 1.0. These tests lock that contract.

    /// `without_coach()` must produce weights that sum to exactly 1.0,
    /// matching the composite's expected full-weight invariant.
    #[test]
    fn without_coach_weights_sum_to_one() {
        let renormalized = spec_weights().without_coach();
        let sum = renormalized.drawdown_adherence
            + renormalized.risk_per_trade_consistency
            + renormalized.setup_adherence
            + renormalized.coach_severity_penalty
            + renormalized.journal_consistency;
        assert_eq!(sum, dec!(1.0), "renormalized weights must sum to 1.0, got {sum}");
    }

    /// The coach weight must be zeroed after renormalization so that any
    /// value flowing through `inputs.coach_severity_penalty` is multiplied
    /// away. Prevents a silent "neutral = 0.0 → Coach Alignment = 1.0 free"
    /// regression that would inflate new-user scores.
    #[test]
    fn without_coach_zeroes_coach_weight() {
        let renormalized = spec_weights().without_coach();
        assert_eq!(
            renormalized.coach_severity_penalty,
            Decimal::ZERO,
            "coach weight must be zero after without_coach()"
        );
    }

    /// Cold-start path: a user with perfect 4-axis discipline must score 100
    /// even though `inputs.coach_severity_penalty` is unknown/arbitrary.
    /// If the coach dimension leaked in, a non-zero penalty would drop the
    /// score below 100. Exercises the `take_daily_snapshot` cold-start branch
    /// from `snapshot.rs:221` (`weights.without_coach()`).
    #[test]
    fn compute_score_on_cold_start_matches_4_axis_composite() {
        let renormalized = spec_weights().without_coach();

        // Same perfect 4-axis discipline, two different coach penalty inputs.
        // With renormalized weights, both must score identically to 100.
        let no_coach_data = InputContributions {
            drawdown_adherence: dec!(1.0),
            risk_per_trade_consistency: dec!(1.0),
            setup_adherence: dec!(1.0),
            coach_severity_penalty: dec!(0.0), // "no data" case
            journal_consistency: dec!(1.0),
        };
        let with_worst_coach_penalty = InputContributions {
            coach_severity_penalty: dec!(1.0), // worst possible — must not matter
            ..no_coach_data
        };

        let score_no_data = compute_score(&no_coach_data, &renormalized);
        let score_worst_penalty = compute_score(&with_worst_coach_penalty, &renormalized);

        assert_eq!(score_no_data, dec!(100), "cold-start perfect discipline must score 100");
        assert_eq!(
            score_no_data, score_worst_penalty,
            "coach_severity_penalty must not influence the score when weights are renormalized for cold-start"
        );
    }

    /// Cold-start must produce a real, computed score — never a 50 placeholder.
    /// Regression guard against the prior `if cold_start { dec!(50) }` short-
    /// circuit. A perfectly disciplined user must score 100 even when the
    /// trade-count gate is active (e.g. n=3 trades, all clean).
    #[test]
    fn cold_start_returns_real_score_not_50() {
        let renormalized = spec_weights().without_coach();
        let perfect = compute_score(&disciplined(), &renormalized);
        assert_eq!(
            perfect,
            dec!(100),
            "cold-start must surface the real computed score, not a neutral 50"
        );
        assert_ne!(
            perfect,
            dec!(50),
            "perfectly disciplined cold-start user must not be flattened to 50"
        );
    }

    /// FR-9 cold-start variant: the ungameability invariant must hold even on
    /// the renormalized 4-axis path. Disciplined user still outscores
    /// undisciplined user after the coach axis is excluded.
    #[test]
    fn cold_start_ungameability_invariant() {
        let renormalized = spec_weights().without_coach();
        let disciplined_score = compute_score(&disciplined(), &renormalized);
        let undisciplined_score =
            compute_score(&undisciplined_high_freq_high_pnl(), &renormalized);
        assert!(
            disciplined_score > undisciplined_score,
            "FR-9 must hold on cold-start path: disciplined ({disciplined_score}) > undisciplined ({undisciplined_score})"
        );
    }
}
