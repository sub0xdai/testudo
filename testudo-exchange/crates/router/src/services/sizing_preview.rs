//! Shared sizing-preview pipeline — QNT-01b byte-parity contract.
//!
//! Both `POST /api/v1/trades` (execution) and `POST /api/v1/trades/preview`
//! (preview) invoke `compute_sizing_preview` so that the effective risk
//! percent, edge multiplier, and kelly_inputs blob rendered in the
//! Alt+X modal match what will be journaled on the executed trade.
//!
//! Branch structure (mirrors QNT-01a T6 in `create_trade`):
//! - `!dynamic_enabled`                         → `FixedMode`   (effective = baseline, mult = 1.0)
//! - `dynamic_enabled && setup_tag.is_none()`   → `Untagged`    (effective = baseline, mult = 1.0; info log)
//! - `dynamic_enabled && full_kelly <= 0`       → `NegativeEdge` (effective = 0,       mult = 0.0)
//! - `dynamic_enabled && full_kelly > 0`        → `Calibrated`   (effective in [0.25×,2×] baseline)
//!
//! Any `sqlx::Error` propagated from calibration loads surfaces to the
//! caller. `create_trade` maps errors to a silent `warn!` + baseline
//! fall-through (preserving QNT-01a "never fail a trade for a
//! calibration hiccup" semantics); the preview route maps errors to 5xx.

use chrono::Utc;
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::services::calibration::{self, CalibrationEngine, SetupStats, ShrunkStats};
use common_utils::risk::kelly::{
    edge_multiplier, effective_risk_percent, full_kelly, PSEUDOCOUNT_K,
};

/// Discriminated reasoning variants returned alongside the sizing numbers.
///
/// Wire format uses `snake_case` tags (matches the extension's
/// `SizingPreviewSchema` Zod discriminated union in T4).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SizingReasoning {
    Calibrated {
        n_setup: u32,
        p_eff: Decimal,
        avg_r_win: Decimal,
        avg_r_loss: Decimal,
    },
    Untagged,
    NegativeEdge {
        quarter_kelly: Decimal,
    },
    FixedMode,
}

/// Preview payload carried both to the HTTP response and the trade
/// execution path.
///
/// `kelly_inputs` is the DB-persistence blob for `journal_trades`; it is
/// `#[serde(skip_serializing)]` because the preview endpoint must not
/// leak internal provenance fields. T3 renders a thin response DTO that
/// omits it; T2 callers in `create_trade` read it directly.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SizingPreview {
    pub baseline_risk_pct: Decimal,
    pub effective_risk_pct: Decimal,
    pub edge_multiplier: Decimal,
    pub reasoning: SizingReasoning,
    #[serde(skip_serializing)]
    pub kelly_inputs: Option<serde_json::Value>,
}

/// Dynamic-risk OFF — baseline flows through unchanged, no kelly blob.
pub fn fixed_mode(baseline: Decimal) -> SizingPreview {
    SizingPreview {
        baseline_risk_pct: baseline,
        effective_risk_pct: baseline,
        edge_multiplier: Decimal::ONE,
        reasoning: SizingReasoning::FixedMode,
        kelly_inputs: None,
    }
}

/// Dynamic-risk ON but setup_tag missing — baseline flows through,
/// no kelly blob (FR-8 silent fallback).
pub fn untagged(baseline: Decimal) -> SizingPreview {
    SizingPreview {
        baseline_risk_pct: baseline,
        effective_risk_pct: baseline,
        edge_multiplier: Decimal::ONE,
        reasoning: SizingReasoning::Untagged,
        kelly_inputs: None,
    }
}

/// Pure classifier for the calibrated branch. Consumes loaded stats +
/// shrunk blend + baseline and decides between `NegativeEdge` and
/// `Calibrated`. Pure so T2 can unit-test every outcome without
/// spinning a Postgres fixture.
pub fn classify_calibrated(
    prior: &SetupStats,
    setup_stats: &SetupStats,
    shrunk: &ShrunkStats,
    baseline: Decimal,
) -> SizingPreview {
    let fk = full_kelly(shrunk.p_eff, shrunk.avg_r_win, shrunk.avg_r_loss);
    let qk = fk / Decimal::from(4);

    if fk <= Decimal::ZERO {
        return SizingPreview {
            baseline_risk_pct: baseline,
            effective_risk_pct: Decimal::ZERO,
            edge_multiplier: Decimal::ZERO,
            reasoning: SizingReasoning::NegativeEdge { quarter_kelly: qk },
            kelly_inputs: None,
        };
    }

    let mult = edge_multiplier(qk);
    let eff = effective_risk_percent(baseline, mult);

    let kelly_inputs = serde_json::json!({
        "mode": "calibrated_kelly",
        "baseline_risk_pct": baseline,
        "effective_risk_pct": eff,
        "edge_multiplier": mult,
        "p_eff": shrunk.p_eff,
        "avg_r_win": shrunk.avg_r_win,
        "avg_r_loss": shrunk.avg_r_loss,
        "quarter_kelly": qk,
        "n_setup": shrunk.n_setup,
        "n_global": shrunk.n_global,
        "pseudocount_k": PSEUDOCOUNT_K,
        "p_setup_raw": setup_stats.p_win,
        "p_global_raw": prior.p_win,
        "computed_at": Utc::now().to_rfc3339(),
    });

    SizingPreview {
        baseline_risk_pct: baseline,
        effective_risk_pct: eff,
        edge_multiplier: mult,
        reasoning: SizingReasoning::Calibrated {
            n_setup: shrunk.n_setup,
            p_eff: shrunk.p_eff,
            avg_r_win: shrunk.avg_r_win,
            avg_r_loss: shrunk.avg_r_loss,
        },
        kelly_inputs: Some(kelly_inputs),
    }
}

/// Load prior + per-setup aggregates, shrink, and classify. The single
/// entry point both `create_trade` and `preview_trade_sizing` call.
///
/// Errors from `load_prior` / `load_setup` are propagated unchanged —
/// callers decide whether to swallow (execution path: warn + fall through
/// to baseline) or surface (preview path: return 5xx).
pub async fn compute_sizing_preview(
    user_id: Uuid,
    setup_tag: Option<&str>,
    baseline_risk_pct: Decimal,
    dynamic_enabled: bool,
    calibration_engine: Option<&CalibrationEngine>,
) -> Result<SizingPreview, sqlx::Error> {
    if !dynamic_enabled {
        return Ok(fixed_mode(baseline_risk_pct));
    }

    let Some(engine) = calibration_engine else {
        // Dynamic on but engine not wired — behave as fixed-mode for
        // sizing purposes. Shouldn't happen in production.
        return Ok(fixed_mode(baseline_risk_pct));
    };

    let Some(tag) = setup_tag else {
        // FR-8: Dynamic on but no setup_tag → silent fallback to baseline.
        tracing::info!(
            user_id = %user_id,
            "QNT-01a: dynamic_risk on but setup_tag missing — falling back to baseline"
        );
        return Ok(untagged(baseline_risk_pct));
    };

    let prior = engine.load_prior(user_id).await?;
    let setup_stats = engine.load_setup(user_id, tag).await?;
    let shrunk = calibration::shrink(&setup_stats, &prior, PSEUDOCOUNT_K);

    Ok(classify_calibrated(
        &prior,
        &setup_stats,
        &shrunk,
        baseline_risk_pct,
    ))
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

    #[tokio::test]
    async fn fixed_mode_returns_baseline_unchanged() {
        let preview = compute_sizing_preview(
            Uuid::new_v4(),
            Some("breakout"),
            dec!(1.0),
            false,
            None,
        )
        .await
        .unwrap();

        assert_eq!(preview.baseline_risk_pct, dec!(1.0));
        assert_eq!(preview.effective_risk_pct, dec!(1.0));
        assert_eq!(preview.edge_multiplier, Decimal::ONE);
        assert!(matches!(preview.reasoning, SizingReasoning::FixedMode));
        assert!(preview.kelly_inputs.is_none());
    }

    #[tokio::test]
    async fn missing_engine_falls_back_to_fixed_mode() {
        // Dynamic on but no engine wired — treat as fixed-mode.
        let preview =
            compute_sizing_preview(Uuid::new_v4(), Some("breakout"), dec!(1.0), true, None)
                .await
                .unwrap();
        assert!(matches!(preview.reasoning, SizingReasoning::FixedMode));
    }

    #[test]
    fn fixed_mode_helper_produces_pass_through() {
        let p = fixed_mode(dec!(1.5));
        assert_eq!(p.baseline_risk_pct, dec!(1.5));
        assert_eq!(p.effective_risk_pct, dec!(1.5));
        assert_eq!(p.edge_multiplier, Decimal::ONE);
        assert!(matches!(p.reasoning, SizingReasoning::FixedMode));
        assert!(p.kelly_inputs.is_none());
    }

    #[test]
    fn untagged_helper_produces_pass_through() {
        let p = untagged(dec!(2.0));
        assert_eq!(p.baseline_risk_pct, dec!(2.0));
        assert_eq!(p.effective_risk_pct, dec!(2.0));
        assert_eq!(p.edge_multiplier, Decimal::ONE);
        assert!(matches!(p.reasoning, SizingReasoning::Untagged));
        assert!(p.kelly_inputs.is_none());
    }

    #[test]
    fn classify_negative_edge_returns_zero_and_no_blob() {
        // p=0.3, b=1 → fk = (1·0.3 − 0.7) / 1 = −0.4 < 0
        let prior = stats(100, dec!(0.3), dec!(1.0), dec!(1.0));
        let setup = stats(100, dec!(0.3), dec!(1.0), dec!(1.0));
        let shrunk = calibration::shrink(&setup, &prior, PSEUDOCOUNT_K);

        let preview = classify_calibrated(&prior, &setup, &shrunk, dec!(1.0));

        assert_eq!(preview.effective_risk_pct, Decimal::ZERO);
        assert_eq!(preview.edge_multiplier, Decimal::ZERO);
        assert!(matches!(
            preview.reasoning,
            SizingReasoning::NegativeEdge { .. }
        ));
        assert!(preview.kelly_inputs.is_none());
    }

    #[test]
    fn classify_calibrated_within_clamp_bounds() {
        // High-edge setup: p=0.70, Rw/Rl=2.0
        let prior = stats(200, dec!(0.55), dec!(1.5), dec!(1.0));
        let setup = stats(100, dec!(0.70), dec!(2.0), dec!(1.0));
        let shrunk = calibration::shrink(&setup, &prior, PSEUDOCOUNT_K);

        let baseline = dec!(1.0);
        let preview = classify_calibrated(&prior, &setup, &shrunk, baseline);

        // Effective risk must be within [0.25×, 2×] the baseline.
        assert!(preview.effective_risk_pct >= dec!(0.25));
        assert!(preview.effective_risk_pct <= dec!(2.00));
        assert!(matches!(
            preview.reasoning,
            SizingReasoning::Calibrated { .. }
        ));
        assert!(preview.kelly_inputs.is_some());

        // kelly_inputs blob carries the expected fields.
        let blob = preview.kelly_inputs.unwrap();
        assert_eq!(blob["mode"], "calibrated_kelly");
        assert!(blob["computed_at"].is_string());
        assert!(blob["pseudocount_k"].as_u64() == Some(PSEUDOCOUNT_K as u64));
    }

    #[test]
    fn classify_calibrated_at_reference_multiplier_is_one() {
        // Stats exactly at reference point (p=0.52, b=1.5) → multiplier = 1 → effective = baseline.
        let prior = stats(100, dec!(0.52), dec!(1.5), dec!(1.0));
        let setup = stats(100, dec!(0.52), dec!(1.5), dec!(1.0));
        let shrunk = calibration::shrink(&setup, &prior, PSEUDOCOUNT_K);

        let baseline = dec!(1.0);
        let preview = classify_calibrated(&prior, &setup, &shrunk, baseline);

        assert_eq!(preview.edge_multiplier, Decimal::ONE);
        assert_eq!(preview.effective_risk_pct, baseline);
    }

    #[test]
    fn calibrated_reasoning_carries_shrunk_stats() {
        let prior = stats(100, dec!(0.50), dec!(1.5), dec!(1.0));
        let setup = stats(50, dec!(0.60), dec!(1.8), dec!(1.0));
        let shrunk = calibration::shrink(&setup, &prior, PSEUDOCOUNT_K);

        let preview = classify_calibrated(&prior, &setup, &shrunk, dec!(1.0));

        if let SizingReasoning::Calibrated {
            n_setup,
            p_eff,
            avg_r_win,
            avg_r_loss,
        } = preview.reasoning
        {
            assert_eq!(n_setup, 50);
            assert_eq!(p_eff, shrunk.p_eff);
            assert_eq!(avg_r_win, shrunk.avg_r_win);
            assert_eq!(avg_r_loss, shrunk.avg_r_loss);
        } else {
            panic!("expected Calibrated reasoning");
        }
    }

    #[test]
    fn negative_edge_reasoning_carries_quarter_kelly() {
        let prior = stats(100, dec!(0.3), dec!(1.0), dec!(1.0));
        let setup = stats(100, dec!(0.3), dec!(1.0), dec!(1.0));
        let shrunk = calibration::shrink(&setup, &prior, PSEUDOCOUNT_K);

        let preview = classify_calibrated(&prior, &setup, &shrunk, dec!(1.0));

        if let SizingReasoning::NegativeEdge { quarter_kelly } = preview.reasoning {
            assert!(quarter_kelly < Decimal::ZERO);
        } else {
            panic!("expected NegativeEdge reasoning");
        }
    }

    #[test]
    fn serialized_reasoning_uses_snake_case_kind_tag() {
        let p = fixed_mode(dec!(1.0));
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["reasoning"]["kind"], "fixed_mode");

        let u = untagged(dec!(1.0));
        let json = serde_json::to_value(&u).unwrap();
        assert_eq!(json["reasoning"]["kind"], "untagged");
    }

    #[test]
    fn serialized_preview_omits_kelly_inputs_field() {
        // The DB-persistence blob must never leak over the wire.
        let prior = stats(100, dec!(0.52), dec!(1.5), dec!(1.0));
        let setup = stats(100, dec!(0.52), dec!(1.5), dec!(1.0));
        let shrunk = calibration::shrink(&setup, &prior, PSEUDOCOUNT_K);

        let preview = classify_calibrated(&prior, &setup, &shrunk, dec!(1.0));
        let json = serde_json::to_value(&preview).unwrap();
        assert!(
            json.get("kelly_inputs").is_none(),
            "kelly_inputs must be skipped in serialized output, got {}",
            json
        );
    }
}
