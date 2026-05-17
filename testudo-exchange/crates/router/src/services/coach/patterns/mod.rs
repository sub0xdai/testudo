//! Deterministic pattern detectors.
//!
//! T3a–T3f each land one detector. The orchestrator `detect_all`
//! composes them into a `Vec<FlaggedPattern>` for the digest.

pub mod correlation_stack;
pub mod frequency_spike;
pub mod session_anomaly;
pub mod setup_fatigue;
pub mod sizing_drift;
pub mod streak_risk;

pub use correlation_stack::detect_correlation_stack;
pub use frequency_spike::detect_frequency_spike;
pub use session_anomaly::detect_session_anomaly;
pub use setup_fatigue::detect_setup_fatigue;
pub use sizing_drift::detect_sizing_drift;
pub use streak_risk::detect_streak_risk;

use super::types::{FlaggedPattern, TradeEvidence, UserBaseline, WeekStats};

/// Run every detector over the week and return the union of flagged patterns.
pub fn detect_all(
    baseline: &UserBaseline,
    trades: &[TradeEvidence],
    stats: &WeekStats,
) -> Vec<FlaggedPattern> {
    let mut out = Vec::new();
    if let Some(p) = detect_sizing_drift(baseline, trades, stats) {
        out.push(p);
    }
    if let Some(p) = detect_frequency_spike(baseline, trades, stats) {
        out.push(p);
    }
    if let Some(p) = detect_session_anomaly(baseline, trades, stats) {
        out.push(p);
    }
    if let Some(p) = detect_setup_fatigue(baseline, trades, stats) {
        out.push(p);
    }
    if let Some(p) = detect_correlation_stack(baseline, trades, stats) {
        out.push(p);
    }
    if let Some(p) = detect_streak_risk(baseline, trades, stats) {
        out.push(p);
    }
    out
}
