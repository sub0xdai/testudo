//! Risk Types
//!
//! Core types for the Decision Loop and risk management system.

// @anchor exchange:common_utils:types
// @tags infra

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Sizing method used for position calculation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizingMethod {
    /// Fixed percentage of account balance at risk
    FixedFractional,
    /// Kelly Criterion based on win rate and average win/loss
    KellyCriterion,
    /// Volatility-adjusted sizing based on ATR
    VolatilityAdjusted,
    /// Maximum risk cap applied
    MaxRiskCap,
    /// QNT-01a: Calibrated Quarter-Kelly with Bayesian shrinkage toward the
    /// user's global prior. Baseline risk percent is overridden by
    /// `effective_risk_percent = baseline × edge_multiplier`, clamped
    /// to `[0.25×, 2.0×]` of baseline.
    CalibratedKelly,
}

impl std::fmt::Display for SizingMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SizingMethod::FixedFractional => write!(f, "Fixed Fractional"),
            SizingMethod::KellyCriterion => write!(f, "Kelly Criterion"),
            SizingMethod::VolatilityAdjusted => write!(f, "Volatility Adjusted"),
            SizingMethod::MaxRiskCap => write!(f, "Max Risk Cap"),
            SizingMethod::CalibratedKelly => write!(f, "Calibrated Kelly"),
        }
    }
}

/// Reasons for rejecting a trade
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "reason", content = "details")]
pub enum RiskRejection {
    /// Account balance insufficient for margin requirement
    InsufficientBalance {
        required: Decimal,
        available: Decimal,
    },
    /// Daily drawdown limit has been exceeded
    DailyDrawdownExceeded {
        current_drawdown_percent: Decimal,
        limit_percent: Decimal,
    },
    /// Maximum number of open positions reached
    MaxPositionsReached { current: u32, maximum: u32 },
    /// Stop loss is required but not provided
    StopLossRequired,
    /// Position size exceeds maximum allowed
    PositionSizeExceeded {
        requested: Decimal,
        maximum: Decimal,
    },
    /// Risk amount exceeds maximum allowed
    RiskAmountExceeded {
        calculated_risk: Decimal,
        maximum: Decimal,
    },
    /// Leverage exceeds maximum allowed
    LeverageExceeded { requested: u8, maximum: u8 },
    /// Risk/reward ratio below minimum
    InsufficientRiskReward {
        calculated: Decimal,
        minimum: Decimal,
    },
}

impl std::fmt::Display for RiskRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskRejection::InsufficientBalance {
                required,
                available,
            } => {
                write!(
                    f,
                    "Insufficient balance: need {}, have {}",
                    required, available
                )
            }
            RiskRejection::DailyDrawdownExceeded {
                current_drawdown_percent,
                limit_percent,
            } => {
                write!(
                    f,
                    "Daily drawdown exceeded: {}% (limit {}%)",
                    current_drawdown_percent, limit_percent
                )
            }
            RiskRejection::MaxPositionsReached { current, maximum } => {
                write!(f, "Max positions reached: {} of {}", current, maximum)
            }
            RiskRejection::StopLossRequired => write!(f, "Stop loss is required"),
            RiskRejection::PositionSizeExceeded { requested, maximum } => {
                write!(f, "Position size exceeded: {} (max {})", requested, maximum)
            }
            RiskRejection::RiskAmountExceeded {
                calculated_risk,
                maximum,
            } => {
                write!(
                    f,
                    "Risk amount exceeded: {} (max {})",
                    calculated_risk, maximum
                )
            }
            RiskRejection::LeverageExceeded { requested, maximum } => {
                write!(f, "Leverage exceeded: {}x (max {}x)", requested, maximum)
            }
            RiskRejection::InsufficientRiskReward {
                calculated,
                minimum,
            } => {
                write!(f, "Risk/reward too low: {} (min {})", calculated, minimum)
            }
        }
    }
}

/// Warnings that don't block trades but inform the user
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "warning", content = "details")]
pub enum RiskWarning {
    /// User-specified size overrides the calculated optimal size
    SizeOverrideExceedsCalculated {
        user_size: Decimal,
        calculated_size: Decimal,
    },
    /// Market volatility is higher than typical
    HighVolatilityDetected {
        current_atr_percent: Decimal,
        typical_atr_percent: Decimal,
    },
    /// Account is approaching daily drawdown limit
    ApproachingDrawdownLimit {
        current_drawdown_percent: Decimal,
        limit_percent: Decimal,
    },
    /// Position is large relative to account
    LargePositionSize {
        position_percent_of_account: Decimal,
    },
    /// Stop loss is very tight
    TightStopLoss { stop_distance_percent: Decimal },
    /// Stop loss is very wide
    WideStopLoss { stop_distance_percent: Decimal },
}

impl std::fmt::Display for RiskWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskWarning::SizeOverrideExceedsCalculated {
                user_size,
                calculated_size,
            } => {
                write!(
                    f,
                    "Size override: {} exceeds calculated {}",
                    user_size, calculated_size
                )
            }
            RiskWarning::HighVolatilityDetected {
                current_atr_percent,
                typical_atr_percent,
            } => {
                write!(
                    f,
                    "High volatility: {}% ATR (typical {}%)",
                    current_atr_percent, typical_atr_percent
                )
            }
            RiskWarning::ApproachingDrawdownLimit {
                current_drawdown_percent,
                limit_percent,
            } => {
                write!(
                    f,
                    "Approaching drawdown limit: {}% (limit {}%)",
                    current_drawdown_percent, limit_percent
                )
            }
            RiskWarning::LargePositionSize {
                position_percent_of_account,
            } => {
                write!(
                    f,
                    "Large position: {}% of account",
                    position_percent_of_account
                )
            }
            RiskWarning::TightStopLoss {
                stop_distance_percent,
            } => {
                write!(f, "Tight stop: {}%", stop_distance_percent)
            }
            RiskWarning::WideStopLoss {
                stop_distance_percent,
            } => {
                write!(f, "Wide stop: {}%", stop_distance_percent)
            }
        }
    }
}

/// Result of risk check and position sizing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskCheckResult {
    /// Whether the trade is approved
    pub approved: bool,
    /// Calculated position size (if approved)
    pub calculated_size: Option<Decimal>,
    /// Which sizing method determined the final size
    pub sizing_method_used: Option<SizingMethod>,
    /// Reason for rejection (if not approved)
    pub rejection_reason: Option<RiskRejection>,
    /// Warnings (even if approved)
    pub warnings: Vec<RiskWarning>,
}

impl RiskCheckResult {
    /// Create an approved result with calculated size
    pub fn approved(size: Decimal, method: SizingMethod) -> Self {
        Self {
            approved: true,
            calculated_size: Some(size),
            sizing_method_used: Some(method),
            rejection_reason: None,
            warnings: Vec::new(),
        }
    }

    /// Create a rejected result with reason
    pub fn rejected(reason: RiskRejection) -> Self {
        Self {
            approved: false,
            calculated_size: None,
            sizing_method_used: None,
            rejection_reason: Some(reason),
            warnings: Vec::new(),
        }
    }

    /// Add a warning to the result
    pub fn with_warning(mut self, warning: RiskWarning) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Add multiple warnings
    pub fn with_warnings(mut self, warnings: Vec<RiskWarning>) -> Self {
        self.warnings.extend(warnings);
        self
    }

    /// Check if there are any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_sizing_method_display() {
        assert_eq!(
            SizingMethod::FixedFractional.to_string(),
            "Fixed Fractional"
        );
        assert_eq!(SizingMethod::KellyCriterion.to_string(), "Kelly Criterion");
        assert_eq!(
            SizingMethod::VolatilityAdjusted.to_string(),
            "Volatility Adjusted"
        );
        assert_eq!(SizingMethod::MaxRiskCap.to_string(), "Max Risk Cap");
    }

    #[test]
    fn test_risk_rejection_display() {
        let rejection = RiskRejection::InsufficientBalance {
            required: dec!(5000),
            available: dec!(1000),
        };
        assert!(rejection.to_string().contains("Insufficient balance"));
        assert!(rejection.to_string().contains("5000"));
        assert!(rejection.to_string().contains("1000"));
    }

    #[test]
    fn test_risk_warning_display() {
        let warning = RiskWarning::HighVolatilityDetected {
            current_atr_percent: dec!(5),
            typical_atr_percent: dec!(2),
        };
        assert!(warning.to_string().contains("High volatility"));
    }

    #[test]
    fn test_risk_check_result_approved() {
        let result = RiskCheckResult::approved(dec!(0.2), SizingMethod::FixedFractional);

        assert!(result.approved);
        assert_eq!(result.calculated_size, Some(dec!(0.2)));
        assert_eq!(
            result.sizing_method_used,
            Some(SizingMethod::FixedFractional)
        );
        assert!(result.rejection_reason.is_none());
        assert!(!result.has_warnings());
    }

    #[test]
    fn test_risk_check_result_rejected() {
        let result = RiskCheckResult::rejected(RiskRejection::StopLossRequired);

        assert!(!result.approved);
        assert!(result.calculated_size.is_none());
        assert!(result.sizing_method_used.is_none());
        assert!(matches!(
            result.rejection_reason,
            Some(RiskRejection::StopLossRequired)
        ));
    }

    #[test]
    fn test_risk_check_result_with_warnings() {
        let result = RiskCheckResult::approved(dec!(0.1), SizingMethod::FixedFractional)
            .with_warning(RiskWarning::TightStopLoss {
                stop_distance_percent: dec!(0.3),
            })
            .with_warning(RiskWarning::LargePositionSize {
                position_percent_of_account: dec!(60),
            });

        assert!(result.approved);
        assert!(result.has_warnings());
        assert_eq!(result.warnings.len(), 2);
    }

    #[test]
    fn test_risk_rejection_serialization() {
        let rejection = RiskRejection::DailyDrawdownExceeded {
            current_drawdown_percent: dec!(6),
            limit_percent: dec!(5),
        };

        let json = serde_json::to_string(&rejection).unwrap();
        // Serde tag uses the variant name
        assert!(json.contains("DailyDrawdownExceeded"));

        let deserialized: RiskRejection = serde_json::from_str(&json).unwrap();
        assert_eq!(rejection, deserialized);
    }

    #[test]
    fn test_sizing_method_serialization() {
        let method = SizingMethod::KellyCriterion;
        let json = serde_json::to_string(&method).unwrap();
        assert_eq!(json, "\"kelly_criterion\"");

        let deserialized: SizingMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(method, deserialized);
    }
}
