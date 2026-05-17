//! Pure input-computation functions for the Dignitas Score pipeline (ENG-01a, T4).
//!
//! Each function maps pre-fetched DB rows → a `[0.0, 1.0]` Decimal contribution.
//! None of these functions touch the database — T5 feeds them pre-loaded data.
//!
//! Division-by-zero guards are mandatory (see AGENTS.md).

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use crate::services::coach::types::Severity;

// ─── Constants ───────────────────────────────────────────────────────────────

/// Number of distinct PatternKind variants.  Used to normalise the per-report
/// coach penalty so that "all 6 patterns at Concerning severity" = 1.0.
const MAX_PATTERNS_PER_REPORT: u32 = 6;

// ─── Input 1: Drawdown Adherence ─────────────────────────────────────────────

/// Fraction of trading days in the trailing window where the observed drawdown
/// stayed within the user's configured daily-max-drawdown limit.
///
/// `days`: slice of `(observed_drawdown_pct, limit_pct)` pairs — one per
/// trading day with data.  Both values are percentages (e.g. `2.5` = 2.5%).
///
/// Returns `1.0` when the slice is empty (no data = no breach).
pub fn compute_drawdown_adherence(days: &[(Decimal, Decimal)]) -> Decimal {
    if days.is_empty() {
        return Decimal::ONE;
    }
    let adherent = days
        .iter()
        .filter(|(observed, limit)| observed <= limit)
        .count();
    Decimal::from(adherent) / Decimal::from(days.len())
}

// ─── Input 2: Risk-per-Trade Consistency ─────────────────────────────────────

/// How consistently each trade's actual risk matches the user's configured
/// `account_risk_percent`.
///
/// `trades`: slice of `(actual_risk_frac, configured_risk_frac)` pairs — both
/// in `[0, 1]` (e.g. `0.02` = 2% of account).  Trades where
/// `configured_risk_frac` is zero are skipped (no-op contribution).
///
/// Formula: `1 − mean(min(|actual − configured| / configured, 1.0))`
///
/// Returns `1.0` when the effective slice is empty.
pub fn compute_risk_per_trade_consistency(trades: &[(Decimal, Decimal)]) -> Decimal {
    let deviations: Vec<Decimal> = trades
        .iter()
        .filter_map(|(actual, configured)| {
            if configured.is_zero() {
                return None;
            }
            let deviation = ((*actual - *configured) / *configured).abs();
            Some(deviation.min(Decimal::ONE))
        })
        .collect();

    if deviations.is_empty() {
        return Decimal::ONE;
    }
    let mean_deviation = deviations.iter().sum::<Decimal>() / Decimal::from(deviations.len());
    (Decimal::ONE - mean_deviation).max(Decimal::ZERO)
}

// ─── Input 3: Setup Adherence ────────────────────────────────────────────────

/// Fraction of trades in the trailing window that carry a non-null `setup_tag`.
///
/// `has_tag`: one `bool` per closed trade — `true` iff `setup_tag IS NOT NULL`.
///
/// Returns `1.0` when the slice is empty.
pub fn compute_setup_adherence(has_tag: &[bool]) -> Decimal {
    if has_tag.is_empty() {
        return Decimal::ONE;
    }
    let tagged = has_tag.iter().filter(|&&t| t).count();
    Decimal::from(tagged) / Decimal::from(has_tag.len())
}

// ─── Input 4: Coach Severity Penalty ─────────────────────────────────────────

/// Weighted severity rate across up to 4 weekly coach reports.
///
/// `report_severities`: one inner `Vec<Severity>` per report, containing the
/// severity of every `FlaggedPattern` in that report.
///
/// Per-report score = `(0.5 × notable + 1.0 × concerning) / MAX_PATTERNS_PER_REPORT`,
/// capped at `1.0`.  Final penalty = mean across reports.
///
/// Returns `0.0` when the slice is empty.  Callers must check for the no-data
/// case and invoke `DignitasWeights::without_coach()` before computing the
/// composite — a `0.0` here is a neutral placeholder, not a "clean coach record".
pub fn compute_coach_severity_penalty(report_severities: &[Vec<Severity>]) -> Decimal {
    if report_severities.is_empty() {
        return Decimal::ZERO;
    }
    let denom = Decimal::from(MAX_PATTERNS_PER_REPORT);
    let sum: Decimal = report_severities
        .iter()
        .map(|sev_list| {
            let raw: Decimal = sev_list
                .iter()
                .map(|s| match s {
                    Severity::Info => Decimal::ZERO,
                    Severity::Notable => dec!(0.5),
                    Severity::Concerning => Decimal::ONE,
                })
                .sum();
            (raw / denom).min(Decimal::ONE)
        })
        .sum();
    sum / Decimal::from(report_severities.len())
}

// ─── Input 5: Journal Consistency ────────────────────────────────────────────

/// Fraction of closed trades in the trailing window that have a non-empty
/// `notes` field OR at least one linked `journal_entries` row.
///
/// `has_doc`: one `bool` per closed trade — `true` iff the trade has notes
/// or a linked journal entry.
///
/// Returns `1.0` when the slice is empty.
pub fn compute_journal_consistency(has_doc: &[bool]) -> Decimal {
    if has_doc.is_empty() {
        return Decimal::ONE;
    }
    let documented = has_doc.iter().filter(|&&d| d).count();
    Decimal::from(documented) / Decimal::from(has_doc.len())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ── drawdown_adherence ──────────────────────────────────────────────────

    #[test]
    fn drawdown_adherence_all_within_limit() {
        let days = vec![(dec!(1.5), dec!(2.0)), (dec!(0.8), dec!(2.0))];
        assert_eq!(compute_drawdown_adherence(&days), Decimal::ONE);
    }

    #[test]
    fn drawdown_adherence_half_breached() {
        let days = vec![
            (dec!(1.5), dec!(2.0)), // ok
            (dec!(2.5), dec!(2.0)), // breach
        ];
        assert_eq!(compute_drawdown_adherence(&days), dec!(0.5));
    }

    #[test]
    fn drawdown_adherence_all_breached() {
        let days = vec![(dec!(3.0), dec!(2.0)), (dec!(4.0), dec!(2.0))];
        assert_eq!(compute_drawdown_adherence(&days), Decimal::ZERO);
    }

    #[test]
    fn drawdown_adherence_empty_is_one() {
        assert_eq!(compute_drawdown_adherence(&[]), Decimal::ONE);
    }

    #[test]
    fn drawdown_adherence_exact_limit_is_adherent() {
        let days = vec![(dec!(2.0), dec!(2.0))];
        assert_eq!(compute_drawdown_adherence(&days), Decimal::ONE);
    }

    // ── risk_per_trade_consistency ──────────────────────────────────────────

    #[test]
    fn risk_consistency_perfect_match() {
        let trades = vec![(dec!(0.02), dec!(0.02)), (dec!(0.02), dec!(0.02))];
        assert_eq!(compute_risk_per_trade_consistency(&trades), Decimal::ONE);
    }

    #[test]
    fn risk_consistency_double_the_configured() {
        // actual = 0.04, configured = 0.02 → |0.04-0.02|/0.02 = 1.0 capped at 1.0
        let trades = vec![(dec!(0.04), dec!(0.02))];
        assert_eq!(compute_risk_per_trade_consistency(&trades), Decimal::ZERO);
    }

    #[test]
    fn risk_consistency_fifty_pct_over() {
        // actual = 0.03, configured = 0.02 → deviation = 0.5 → score = 0.5
        let trades = vec![(dec!(0.03), dec!(0.02))];
        assert_eq!(compute_risk_per_trade_consistency(&trades), dec!(0.5));
    }

    #[test]
    fn risk_consistency_zero_configured_skipped() {
        // configured = 0 → skip → empty → 1.0
        let trades = vec![(dec!(0.02), dec!(0.0))];
        assert_eq!(compute_risk_per_trade_consistency(&trades), Decimal::ONE);
    }

    #[test]
    fn risk_consistency_empty_is_one() {
        assert_eq!(compute_risk_per_trade_consistency(&[]), Decimal::ONE);
    }

    #[test]
    fn risk_consistency_deviation_capped_per_trade() {
        // Huge over-sizing: 10× configured. Capped at 1.0 per trade.
        // Two trades: one perfect (dev=0), one 10× (capped dev=1.0) → mean=0.5 → score=0.5
        let trades = vec![(dec!(0.02), dec!(0.02)), (dec!(0.20), dec!(0.02))];
        assert_eq!(compute_risk_per_trade_consistency(&trades), dec!(0.5));
    }

    // ── setup_adherence ─────────────────────────────────────────────────────

    #[test]
    fn setup_adherence_all_tagged() {
        let tags = [true, true, true];
        assert_eq!(compute_setup_adherence(&tags), Decimal::ONE);
    }

    #[test]
    fn setup_adherence_none_tagged() {
        let tags = [false, false, false];
        assert_eq!(compute_setup_adherence(&tags), Decimal::ZERO);
    }

    #[test]
    fn setup_adherence_half_tagged() {
        let tags = [true, false];
        assert_eq!(compute_setup_adherence(&tags), dec!(0.5));
    }

    #[test]
    fn setup_adherence_empty_is_one() {
        assert_eq!(compute_setup_adherence(&[]), Decimal::ONE);
    }

    // ── coach_severity_penalty ──────────────────────────────────────────────

    #[test]
    fn coach_penalty_empty_is_zero() {
        assert_eq!(compute_coach_severity_penalty(&[]), Decimal::ZERO);
    }

    #[test]
    fn coach_penalty_no_flags() {
        let reports = vec![vec![], vec![]];
        assert_eq!(compute_coach_severity_penalty(&reports), Decimal::ZERO);
    }

    #[test]
    fn coach_penalty_all_concerning_one_report() {
        // 6 Concerning in 1 report → raw=6.0/6=1.0 → penalty=1.0
        let reports = vec![vec![
            Severity::Concerning,
            Severity::Concerning,
            Severity::Concerning,
            Severity::Concerning,
            Severity::Concerning,
            Severity::Concerning,
        ]];
        assert_eq!(compute_coach_severity_penalty(&reports), Decimal::ONE);
    }

    #[test]
    fn coach_penalty_one_notable_one_report() {
        // 1 Notable → raw = 0.5/6 ≈ 0.0833
        let reports = vec![vec![Severity::Notable]];
        let result = compute_coach_severity_penalty(&reports);
        let expected = dec!(0.5) / Decimal::from(6u32);
        assert_eq!(result, expected);
    }

    #[test]
    fn coach_penalty_mixed_two_reports() {
        // report1: 1 Concerning → raw=1/6; report2: 1 Notable → raw=0.5/6
        // mean = ((1/6) + (0.5/6)) / 2 = 1.5/12 = 0.125
        let reports = vec![
            vec![Severity::Concerning],
            vec![Severity::Notable],
        ];
        let result = compute_coach_severity_penalty(&reports);
        let expected = (Decimal::ONE / Decimal::from(6u32) + dec!(0.5) / Decimal::from(6u32))
            / dec!(2);
        assert_eq!(result, expected);
    }

    #[test]
    fn coach_penalty_info_severity_ignored() {
        let reports = vec![vec![Severity::Info, Severity::Info]];
        assert_eq!(compute_coach_severity_penalty(&reports), Decimal::ZERO);
    }

    #[test]
    fn coach_penalty_capped_at_one_per_report() {
        // 12 Concerning in 1 report → raw=12/6=2.0, capped to 1.0
        let reports = vec![vec![Severity::Concerning; 12]];
        assert_eq!(compute_coach_severity_penalty(&reports), Decimal::ONE);
    }

    // ── journal_consistency ─────────────────────────────────────────────────

    #[test]
    fn journal_consistency_all_documented() {
        assert_eq!(compute_journal_consistency(&[true, true, true]), Decimal::ONE);
    }

    #[test]
    fn journal_consistency_none_documented() {
        assert_eq!(compute_journal_consistency(&[false, false]), Decimal::ZERO);
    }

    #[test]
    fn journal_consistency_half_documented() {
        assert_eq!(compute_journal_consistency(&[true, false]), dec!(0.5));
    }

    #[test]
    fn journal_consistency_empty_is_one() {
        assert_eq!(compute_journal_consistency(&[]), Decimal::ONE);
    }
}
