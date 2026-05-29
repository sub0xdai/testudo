//! Calibration engine — per-setup and user-level prior aggregates for the
//! Calibrated Kelly sizing path (QNT-01a).
//!
//! Pure Bayesian shrinkage (`shrink`) lives here alongside the two I/O
//! helpers that load the per-setup and user-global aggregates. The
//! pseudocount constant `PSEUDOCOUNT_K` is re-exported from
//! `common_utils::risk::kelly` so there's a single source of truth.
//!
//! Spec placement deviates from `common_utils` (per QNT-01a planning
//! Discovery #3): common_utils is I/O-free by convention, so the sqlx-bearing
//! engine lives here next to the other router services.
//!
//! # Conventions
//! - `p_win` = closed-trade fraction where `net_pnl > 0` (denominator
//!   includes trades with or without an `r_multiple`).
//! - `avg_r_win` = mean `r_multiple` across winning trades whose
//!   `r_multiple IS NOT NULL` (the only trades that contribute an R to
//!   the average).
//! - `avg_r_loss` = mean `|r_multiple|` across losing trades whose
//!   `r_multiple IS NOT NULL` (positive magnitude).
//! - Setup match is case-insensitive (`LOWER(setup_tag) = LOWER($2)`) to
//!   agree with RSK-02 `setup_breakdown` normalization.

// @anchor exchange:router:calibration
// @tags api

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

pub use common_utils::risk::kelly::PSEUDOCOUNT_K;

/// Raw aggregate over a single setup (or the user's global tagged history
/// when loaded as the prior).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupStats {
    pub n: u32,
    pub p_win: Decimal,
    pub avg_r_win: Decimal,
    pub avg_r_loss: Decimal,
}

/// Bayesian-shrunk aggregate: per-setup stats blended with the user's
/// global prior at pseudocount `K`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShrunkStats {
    pub p_eff: Decimal,
    pub avg_r_win: Decimal,
    pub avg_r_loss: Decimal,
    pub n_setup: u32,
    pub n_global: u32,
}

/// I/O-bearing loader for calibration aggregates. Held as `Arc` in
/// `AppState` and consumed by `create_trade` when dynamic risk is enabled.
pub struct CalibrationEngine {
    pool: PgPool,
}

#[derive(Debug, sqlx::FromRow)]
struct AggRow {
    n: i64,
    p_win: Decimal,
    avg_r_win: Decimal,
    avg_r_loss: Decimal,
}

impl CalibrationEngine {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Load the user's global prior across **all tagged** closed trades.
    /// Untagged trades are excluded — the prior is the user's own tagged
    /// history, which matches the QNT-01a thesis that untagged activity
    /// should not calibrate tagged sizing.
    pub async fn load_prior(&self, user_id: Uuid) -> Result<SetupStats, sqlx::Error> {
        let row = sqlx::query_as::<_, AggRow>(
            "SELECT \
                COUNT(*)::BIGINT AS n, \
                COALESCE( \
                    (COUNT(*) FILTER (WHERE net_pnl > 0))::NUMERIC \
                    / GREATEST(COUNT(*), 1), 0 \
                )::NUMERIC AS p_win, \
                COALESCE( \
                    AVG(r_multiple) FILTER (WHERE net_pnl > 0 AND r_multiple IS NOT NULL), \
                    0 \
                )::NUMERIC AS avg_r_win, \
                COALESCE( \
                    AVG(ABS(r_multiple)) FILTER (WHERE net_pnl <= 0 AND r_multiple IS NOT NULL), \
                    0 \
                )::NUMERIC AS avg_r_loss \
            FROM journal_trades \
            WHERE user_id = $1 \
                AND setup_tag IS NOT NULL \
                AND closed_at IS NOT NULL",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row_to_stats(row))
    }

    /// Load per-setup aggregates for `(user_id, LOWER(setup_tag))`.
    /// Case-insensitive match; empty/whitespace-only tags should already
    /// be normalized to `None` by the RSK-02 T1 entry-point filter.
    pub async fn load_setup(
        &self,
        user_id: Uuid,
        setup_tag: &str,
    ) -> Result<SetupStats, sqlx::Error> {
        let row = sqlx::query_as::<_, AggRow>(
            "SELECT \
                COUNT(*)::BIGINT AS n, \
                COALESCE( \
                    (COUNT(*) FILTER (WHERE net_pnl > 0))::NUMERIC \
                    / GREATEST(COUNT(*), 1), 0 \
                )::NUMERIC AS p_win, \
                COALESCE( \
                    AVG(r_multiple) FILTER (WHERE net_pnl > 0 AND r_multiple IS NOT NULL), \
                    0 \
                )::NUMERIC AS avg_r_win, \
                COALESCE( \
                    AVG(ABS(r_multiple)) FILTER (WHERE net_pnl <= 0 AND r_multiple IS NOT NULL), \
                    0 \
                )::NUMERIC AS avg_r_loss \
            FROM journal_trades \
            WHERE user_id = $1 \
                AND LOWER(setup_tag) = LOWER($2) \
                AND closed_at IS NOT NULL",
        )
        .bind(user_id)
        .bind(setup_tag)
        .fetch_one(&self.pool)
        .await?;

        Ok(row_to_stats(row))
    }
}

fn row_to_stats(row: AggRow) -> SetupStats {
    SetupStats {
        n: row.n.max(0) as u32,
        p_win: row.p_win,
        avg_r_win: row.avg_r_win,
        avg_r_loss: row.avg_r_loss,
    }
}

/// Blend per-setup stats with the user's global prior at pseudocount `k`.
///
/// ```text
/// p_eff       = (N_s · p_s + K · p_g) / (N_s + K)
/// avg_r_win   = (N_s · R_w_s + K · R_w_g) / (N_s + K)
/// avg_r_loss  = (N_s · R_l_s + K · R_l_g) / (N_s + K)
/// ```
///
/// At `N_s = 0` the shrunk tuple equals the prior (100% shrinkage).
/// At `N_s = K` it is a 50/50 blend. At `N_s ≫ K` the per-setup
/// aggregates dominate.
pub fn shrink(setup: &SetupStats, prior: &SetupStats, k: u32) -> ShrunkStats {
    let ns = Decimal::from(setup.n);
    let kd = Decimal::from(k);
    let denom = ns + kd;

    if denom.is_zero() {
        // No setup history AND K=0 is a degenerate caller contract.
        // Fall back to the prior to avoid a division-by-zero panic.
        return ShrunkStats {
            p_eff: prior.p_win,
            avg_r_win: prior.avg_r_win,
            avg_r_loss: prior.avg_r_loss,
            n_setup: setup.n,
            n_global: prior.n,
        };
    }

    let p_eff = (ns * setup.p_win + kd * prior.p_win) / denom;
    let avg_r_win = (ns * setup.avg_r_win + kd * prior.avg_r_win) / denom;
    let avg_r_loss = (ns * setup.avg_r_loss + kd * prior.avg_r_loss) / denom;

    ShrunkStats {
        p_eff,
        avg_r_win,
        avg_r_loss,
        n_setup: setup.n,
        n_global: prior.n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn stats(n: u32, p: Decimal, rw: Decimal, rl: Decimal) -> SetupStats {
        SetupStats {
            n,
            p_win: p,
            avg_r_win: rw,
            avg_r_loss: rl,
        }
    }

    #[test]
    fn shrink_at_zero_setup_trades_returns_prior() {
        let setup = stats(0, dec!(0), dec!(0), dec!(0));
        let prior = stats(312, dec!(0.54), dec!(1.8), dec!(1.0));

        let out = shrink(&setup, &prior, PSEUDOCOUNT_K);
        assert_eq!(out.p_eff, prior.p_win);
        assert_eq!(out.avg_r_win, prior.avg_r_win);
        assert_eq!(out.avg_r_loss, prior.avg_r_loss);
        assert_eq!(out.n_setup, 0);
        assert_eq!(out.n_global, 312);
    }

    #[test]
    fn shrink_at_k_equals_fifty_fifty_blend() {
        // N_s = K = 10 → half setup, half prior.
        let setup = stats(10, dec!(0.8), dec!(2.0), dec!(1.0));
        let prior = stats(200, dec!(0.4), dec!(1.0), dec!(1.0));

        let out = shrink(&setup, &prior, 10);
        assert_eq!(out.p_eff, dec!(0.6));
        assert_eq!(out.avg_r_win, dec!(1.5));
        assert_eq!(out.avg_r_loss, dec!(1.0));
    }

    #[test]
    fn shrink_at_large_n_dominates_by_setup() {
        // N_s = 100 vs K = 10 → setup weight 100/110 ≈ 0.909.
        let setup = stats(100, dec!(0.70), dec!(2.00), dec!(1.00));
        let prior = stats(400, dec!(0.50), dec!(1.00), dec!(1.00));

        let out = shrink(&setup, &prior, PSEUDOCOUNT_K);
        // p_eff = (100*0.70 + 10*0.50) / 110 = 75/110 ≈ 0.6818
        let expected_p = dec!(75) / dec!(110);
        let delta = (out.p_eff - expected_p).abs();
        assert!(
            delta < dec!(0.0001),
            "p_eff={} expected≈{} delta={}",
            out.p_eff,
            expected_p,
            delta
        );
        // Output must be closer to the setup than to the prior.
        let dist_to_setup = (out.p_eff - setup.p_win).abs();
        let dist_to_prior = (out.p_eff - prior.p_win).abs();
        assert!(dist_to_setup < dist_to_prior);
    }

    #[test]
    fn anti_gaming_small_n_cannot_spike_p_eff() {
        // A single trade claiming p_win=1.0 on a neutral prior should be
        // pulled strongly toward the prior — not enough evidence to fire
        // Kelly at a 100% win rate.
        let setup = stats(1, dec!(1.0), dec!(3.0), dec!(1.0));
        let prior = stats(500, dec!(0.50), dec!(1.0), dec!(1.0));

        let out = shrink(&setup, &prior, PSEUDOCOUNT_K);
        // p_eff = (1*1.0 + 10*0.5) / 11 = 6/11 ≈ 0.545
        let expected = dec!(6) / dec!(11);
        let delta = (out.p_eff - expected).abs();
        assert!(
            delta < dec!(0.0001),
            "p_eff={} expected≈{} delta={}",
            out.p_eff,
            expected,
            delta
        );
        // Must be well under 0.60 — a cold-start user can't game the
        // engine into aggressive sizing off a single win.
        assert!(
            out.p_eff < dec!(0.60),
            "p_eff should be shrunk toward prior, got {}",
            out.p_eff
        );
    }

    #[test]
    fn shrink_preserves_counts_in_output() {
        let setup = stats(42, dec!(0.6), dec!(1.8), dec!(0.9));
        let prior = stats(312, dec!(0.5), dec!(1.2), dec!(0.8));

        let out = shrink(&setup, &prior, PSEUDOCOUNT_K);
        assert_eq!(out.n_setup, 42);
        assert_eq!(out.n_global, 312);
    }

    #[test]
    fn shrink_handles_zero_prior_and_zero_setup() {
        // Cold-start user: no tagged history at all. Result must not
        // panic; all shrunk values fall to 0 (caller treats full_kelly
        // ≤ 0 as negative-edge rejection elsewhere).
        let setup = stats(0, dec!(0), dec!(0), dec!(0));
        let prior = stats(0, dec!(0), dec!(0), dec!(0));

        let out = shrink(&setup, &prior, PSEUDOCOUNT_K);
        assert_eq!(out.p_eff, Decimal::ZERO);
        assert_eq!(out.avg_r_win, Decimal::ZERO);
        assert_eq!(out.avg_r_loss, Decimal::ZERO);
    }

    #[test]
    fn shrink_with_zero_k_and_zero_n_falls_back_to_prior() {
        // Degenerate guard: denom = 0, avoid division-by-zero panic.
        let setup = stats(0, dec!(0), dec!(0), dec!(0));
        let prior = stats(7, dec!(0.6), dec!(1.5), dec!(1.0));

        let out = shrink(&setup, &prior, 0);
        assert_eq!(out.p_eff, prior.p_win);
        assert_eq!(out.avg_r_win, prior.avg_r_win);
        assert_eq!(out.avg_r_loss, prior.avg_r_loss);
    }

    #[test]
    fn pseudocount_constant_is_ten() {
        // Lock the single source of truth — any future tuning that
        // touches `common_utils::risk::kelly::PSEUDOCOUNT_K` surfaces
        // here as a test failure.
        assert_eq!(PSEUDOCOUNT_K, 10);
    }
}
