// @anchor infra:cli:risk:precheck
// @tags infra

//! Client-side risk pre-check — validates signals before API submission.
//!
//! Catches common LLM mistakes (over-leveraged, disallowed symbols, max positions)
//! before the signal hits the backend, saving API calls and providing the LLM
//! with actionable feedback via tool result responses.

use crate::strategies::template::StrategyConstraints;

/// Result of a pre-check validation.
#[derive(Debug, Clone)]
pub struct PrecheckResult {
    pub passed: bool,
    pub reason: Option<String>,
    pub suggestion: Option<String>,
}

impl PrecheckResult {
    fn pass() -> Self {
        Self {
            passed: true,
            reason: None,
            suggestion: None,
        }
    }

    fn fail(reason: String, suggestion: Option<String>) -> Self {
        Self {
            passed: false,
            reason: Some(reason),
            suggestion,
        }
    }
}

/// Client-side risk validator.
///
/// Validates signals against strategy constraints before they reach the backend.
/// The pre-check is a subset of the backend's RiskService — it catches common
/// violations early but the backend remains the final authority.
pub struct RiskPrecheck {
    constraints: StrategyConstraints,
    position_count: usize,
    max_positions: usize,
}

impl RiskPrecheck {
    /// Create a new pre-check with strategy constraints and current position state.
    pub fn new(
        constraints: &StrategyConstraints,
        position_count: usize,
        max_positions: usize,
    ) -> Self {
        Self {
            constraints: constraints.clone(),
            position_count,
            max_positions,
        }
    }

    /// Validate leverage against strategy's max_leverage.
    /// If no max is set, any leverage passes.
    pub fn validate_leverage(&self, leverage: u8) -> PrecheckResult {
        if let Some(max_lev) = self.constraints.max_leverage {
            if leverage > max_lev {
                return PrecheckResult::fail(
                    format!(
                        "Leverage {}× exceeds strategy maximum of {}×",
                        leverage, max_lev
                    ),
                    Some(format!("Reduce leverage to {}× or lower", max_lev)),
                );
            }
        }
        PrecheckResult::pass()
    }

    /// Validate that max positions haven't been exceeded.
    pub fn validate_positions(&self) -> PrecheckResult {
        if self.position_count >= self.max_positions {
            return PrecheckResult::fail(
                format!(
                    "Max positions reached ({}/{}). Close a position before opening a new one.",
                    self.position_count, self.max_positions
                ),
                Some(
                    "Wait for an existing position to close or increase max_positions \
                     in strategy config"
                        .into(),
                ),
            );
        }
        PrecheckResult::pass()
    }

    /// Validate that a symbol is in the strategy's allowed list.
    /// If no allowed list is set, any symbol passes.
    pub fn validate_symbol(&self, symbol: &str) -> PrecheckResult {
        if let Some(ref allowed) = self.constraints.allowed_symbols {
            if !allowed.iter().any(|s| s == symbol) {
                return PrecheckResult::fail(
                    format!(
                        "Symbol '{}' is not in the strategy's allowed list: {}",
                        symbol,
                        allowed.join(", ")
                    ),
                    Some(
                        "Use an allowed symbol or update the strategy constraints".into(),
                    ),
                );
            }
        }
        PrecheckResult::pass()
    }
}
