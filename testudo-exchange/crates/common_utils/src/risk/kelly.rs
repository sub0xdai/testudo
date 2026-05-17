//! Calibrated Kelly sizing — pure math helpers.
//!
//! Implements baseline-anchored Quarter-Kelly with a ±2× clamp around a
//! reference Quarter-Kelly point. All functions are pure (no I/O, no
//! allocation beyond Decimal). The I/O-bearing baseline loader and
//! Bayesian shrinkage live in `router/src/services/calibration.rs`; the
//! pseudocount constant used there is re-exported from this module so
//! there is a single source of truth.
//!
//! # Math (verbatim from QNT-01a-kelly-engine §Mathematics)
//!
//! ```text
//! b = avg_r_win / avg_r_loss        // reward-to-risk ratio in R-multiples
//! p = p_eff
//! q = 1 - p
//!
//! full_kelly    = (b × p - q) / b
//! quarter_kelly = full_kelly / 4
//!
//! edge_multiplier        = clamp(quarter_kelly / reference_kelly, 0.25, 2.0)
//! effective_risk_percent = baseline_risk_percent × edge_multiplier
//! ```
//!
//! `reference_kelly` is Quarter-Kelly evaluated at `(p = 0.52, b = 1.5)` —
//! a "typical disciplined setup". At that point `edge_multiplier == 1`,
//! so the baseline fixed-fractional risk flows through unchanged.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::OnceLock;

/// Bayesian pseudocount used when blending per-setup stats with the
/// user's global prior. Single source of truth — consumed by
/// `router::services::calibration::shrink`.
pub const PSEUDOCOUNT_K: u32 = 10;

/// Minimum edge multiplier — effective risk never drops below 25% of baseline.
pub const CLAMP_MIN: Decimal = dec!(0.25);

/// Maximum edge multiplier — effective risk never exceeds 2× baseline.
pub const CLAMP_MAX: Decimal = dec!(2.00);

/// Quarter-Kelly at the reference point `(p = 0.52, b = 1.5)`.
///
/// Computed once and cached. With the spec's formula this evaluates to
/// exactly `0.05` (5% of bankroll per trade at a typical setup). The
/// ratio `quarter_kelly / reference_kelly` is the edge multiplier, so
/// only the consistent use of the same formula on both sides matters —
/// the absolute magnitude cancels.
pub fn reference_kelly() -> Decimal {
    static CACHE: OnceLock<Decimal> = OnceLock::new();
    *CACHE.get_or_init(|| quarter_kelly(dec!(0.52), dec!(1.5), dec!(1.0)))
}

/// Full Kelly fraction = `(b·p − q) / b`, where `b = avg_r_win / avg_r_loss`.
///
/// Returns the raw value — may be negative if the edge is negative or
/// zero if `avg_r_loss <= 0` (guard against division-by-zero; caller
/// should treat non-positive return as "no position").
pub fn full_kelly(p_eff: Decimal, avg_r_win: Decimal, avg_r_loss: Decimal) -> Decimal {
    if avg_r_loss <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    let b = avg_r_win / avg_r_loss;
    if b <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    let q = Decimal::ONE - p_eff;
    (b * p_eff - q) / b
}

/// Quarter-Kelly = `full_kelly / 4`.
pub fn quarter_kelly(p_eff: Decimal, avg_r_win: Decimal, avg_r_loss: Decimal) -> Decimal {
    full_kelly(p_eff, avg_r_win, avg_r_loss) / dec!(4)
}

/// Edge multiplier = `clamp(quarter_kelly / reference_kelly, 0.25, 2.0)`.
///
/// Callers must treat a non-positive `quarter_kelly` as a rejection
/// signal before calling this — the clamp would otherwise mask a
/// negative edge by pinning the multiplier at `CLAMP_MIN`.
pub fn edge_multiplier(quarter_kelly: Decimal) -> Decimal {
    let reference = reference_kelly();
    if reference <= Decimal::ZERO {
        return CLAMP_MIN;
    }
    let raw = quarter_kelly / reference;
    raw.clamp(CLAMP_MIN, CLAMP_MAX)
}

/// Effective per-trade risk percent = `baseline × multiplier`.
pub fn effective_risk_percent(baseline: Decimal, multiplier: Decimal) -> Decimal {
    baseline * multiplier
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quarter-Kelly at the reference point evaluates to exactly 0.05
    /// under the spec's formula. (The spec brainstorm's inline comment
    /// "≈ 0.0133" is a calculation error in prose — the formula is the
    /// source of truth, and only the ratio `qk / reference_kelly`
    /// matters for the edge multiplier anyway.)
    #[test]
    fn reference_kelly_evaluates_to_expected_value() {
        let reference = reference_kelly();
        let expected = dec!(0.05);
        let delta = (reference - expected).abs();
        assert!(
            delta < dec!(0.0001),
            "reference_kelly={} expected≈{} delta={}",
            reference,
            expected,
            delta
        );
    }

    #[test]
    fn full_kelly_positive_on_positive_edge() {
        // p=0.6, avg_r_win=2, avg_r_loss=1 → b=2, q=0.4
        // full = (2*0.6 - 0.4)/2 = (1.2 - 0.4)/2 = 0.4
        let fk = full_kelly(dec!(0.6), dec!(2), dec!(1));
        assert!(fk > Decimal::ZERO, "expected positive edge, got {}", fk);
        assert_eq!(fk, dec!(0.4));
    }

    #[test]
    fn full_kelly_negative_on_negative_edge() {
        // p=0.4, avg_r_win=1, avg_r_loss=1 → b=1, q=0.6
        // full = (1*0.4 - 0.6)/1 = -0.2
        let fk = full_kelly(dec!(0.4), dec!(1), dec!(1));
        assert!(fk <= Decimal::ZERO, "expected non-positive edge, got {}", fk);
        assert_eq!(fk, dec!(-0.2));
    }

    #[test]
    fn full_kelly_zero_on_zero_loss_guard() {
        // Division-by-zero guard: avg_r_loss == 0 → return 0, not panic.
        let fk = full_kelly(dec!(0.7), dec!(2), Decimal::ZERO);
        assert_eq!(fk, Decimal::ZERO);
    }

    #[test]
    fn edge_multiplier_clamped_at_low() {
        // Tiny quarter_kelly → raw ratio well below CLAMP_MIN → pinned to 0.25.
        let mult = edge_multiplier(dec!(0.001));
        assert_eq!(mult, CLAMP_MIN);
    }

    #[test]
    fn edge_multiplier_clamped_at_high() {
        // Huge quarter_kelly → raw ratio well above CLAMP_MAX → pinned to 2.0.
        let mult = edge_multiplier(dec!(0.5));
        assert_eq!(mult, CLAMP_MAX);
    }

    #[test]
    fn edge_multiplier_is_one_at_reference_point() {
        // Quarter-Kelly at the reference point should produce multiplier 1.0.
        let reference = reference_kelly();
        let mult = edge_multiplier(reference);
        assert_eq!(mult, dec!(1));
    }

    #[test]
    fn effective_risk_percent_matches_baseline_at_multiplier_one() {
        let eff = effective_risk_percent(dec!(1.5), dec!(1));
        assert_eq!(eff, dec!(1.5));
    }

    #[test]
    fn effective_risk_percent_doubles_at_clamp_max() {
        let eff = effective_risk_percent(dec!(1), CLAMP_MAX);
        assert_eq!(eff, dec!(2));
    }

    #[test]
    fn effective_risk_percent_quarters_at_clamp_min() {
        let eff = effective_risk_percent(dec!(1), CLAMP_MIN);
        assert_eq!(eff, dec!(0.25));
    }

    #[test]
    fn quarter_kelly_is_full_kelly_over_four() {
        let fk = full_kelly(dec!(0.6), dec!(2), dec!(1));
        let qk = quarter_kelly(dec!(0.6), dec!(2), dec!(1));
        assert_eq!(qk, fk / dec!(4));
    }
}
