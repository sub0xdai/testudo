//! Task 2: RiskRule trait and MaxTradeRiskRule implementation
//!
//! This module implements a RiskRule trait with an assess method that returns
//! a RiskAssessment, following TDD principles and Roman military discipline.

use crate::types::{TradeProposal, RiskAssessment, ProtocolLimits, ViolationSeverity, ProtocolViolation};
use disciplina::PositionSizingCalculator;
use rust_decimal::Decimal;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur during risk assessment
#[derive(Debug, Error, Clone)]
pub enum AssessmentError {
    #[error("Position sizing calculation failed: {reason}")]
    PositionSizingFailure { reason: String },
    
    #[error("Invalid trade proposal: {reason}")]
    InvalidProposal { reason: String },
    
    #[error("Assessment configuration error: {reason}")]
    ConfigurationError { reason: String },
}

/// Task 2: RiskRule trait with assess method
/// 
/// This trait defines a contract for risk rules that can assess trade proposals
/// and return complete risk assessments rather than just validation results.
pub trait RiskRule: Send + Sync + std::fmt::Debug {
    /// Assess a trade proposal and return a complete risk assessment
    /// 
    /// This method performs comprehensive risk analysis and returns a RiskAssessment
    /// containing position sizing, risk calculations, and approval status.
    fn assess(&self, proposal: &TradeProposal) -> Result<RiskAssessment, AssessmentError>;
    
    /// Get the name of this rule for logging and identification
    fn rule_name(&self) -> &str;
    
    /// Get a description of what this rule assesses
    fn description(&self) -> &str;
}

/// Task 2: MaxTradeRiskRule implementation
/// 
/// This rule validates that individual trade risk does not exceed the maximum
/// allowed percentage of account equity as defined by the Testudo Protocol.
#[derive(Debug, Clone)]
pub struct MaxTradeRiskRule {
    /// Protocol limits for risk validation
    limits: ProtocolLimits,
    /// Van Tharp position sizing calculator
    position_calculator: Arc<PositionSizingCalculator>,
}

impl MaxTradeRiskRule {
    /// Create a new MaxTradeRiskRule with default protocol limits
    pub fn new() -> Self {
        Self::with_limits(ProtocolLimits::default())
    }
    
    /// Create a new MaxTradeRiskRule with custom protocol limits
    pub fn with_limits(limits: ProtocolLimits) -> Self {
        Self {
            limits,
            position_calculator: Arc::new(PositionSizingCalculator::new()),
        }
    }
    
    /// Create a conservative MaxTradeRiskRule for new traders
    pub fn conservative() -> Self {
        Self::with_limits(ProtocolLimits::conservative_limits())
    }
    
    /// Create an aggressive MaxTradeRiskRule for experienced traders  
    pub fn aggressive() -> Self {
        Self::with_limits(ProtocolLimits::aggressive_limits())
    }
}

impl RiskRule for MaxTradeRiskRule {
    fn assess(&self, proposal: &TradeProposal) -> Result<RiskAssessment, AssessmentError> {
        // Step 1: Calculate position size using Van Tharp methodology
        let position_size = self.position_calculator
            .calculate_position_size(
                proposal.account_equity.value(),
                proposal.risk_percentage.value(),
                proposal.entry_price.value(),
                proposal.stop_loss.value(),
            )
            .map_err(|e| AssessmentError::PositionSizingFailure { 
                reason: e.to_string() 
            })?;
        
        // Step 2: Calculate risk metrics
        let risk_distance = proposal.risk_distance();
        let risk_amount = position_size.value() * risk_distance;
        let portfolio_impact = risk_amount / proposal.account_equity.value();
        
        // Step 3: Create initial assessment
        let mut assessment = RiskAssessment::new(
            proposal.id,
            position_size,
            risk_amount,
            proposal.risk_percentage.value(),
            proposal.risk_reward_ratio(),
            portfolio_impact,
        );
        
        // Step 4: Check if risk exceeds maximum allowed
        if proposal.risk_percentage.value() > self.limits.max_individual_trade_risk {
            let violation = ProtocolViolation::new(
                self.rule_name().to_string(),
                ViolationSeverity::Critical,
                format!(
                    "Individual trade risk {}% exceeds maximum allowed {}%",
                    proposal.risk_percentage.value() * Decimal::from(100),
                    self.limits.max_individual_trade_risk * Decimal::from(100)
                ),
                proposal.risk_percentage.value(),
                self.limits.max_individual_trade_risk,
                format!(
                    "Reduce position risk to maximum {}% of account equity",
                    self.limits.max_individual_trade_risk * Decimal::from(100)
                ),
            );
            assessment.add_violation(violation);
        }
        
        // Step 5: Add assessment reasoning
        let reasoning = if assessment.is_approved() {
            format!(
                "Trade approved: Position size {} at {}% risk (${} at risk) - within protocol limits",
                assessment.position_size.value(),
                assessment.risk_percentage * Decimal::from(100),
                assessment.risk_amount
            )
        } else {
            format!(
                "Trade rejected: Risk {}% exceeds maximum {}% - position sizing would violate Testudo Protocol",
                proposal.risk_percentage.value() * Decimal::from(100),
                self.limits.max_individual_trade_risk * Decimal::from(100)
            )
        };
        
        Ok(assessment.with_reasoning(reasoning))
    }
    
    fn rule_name(&self) -> &str {
        "MaxTradeRisk"
    }
    
    fn description(&self) -> &str {
        "Validates that individual trade risk does not exceed maximum protocol limit"
    }
}

impl Default for MaxTradeRiskRule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TradeSide, ApprovalStatus};
    use disciplina::{AccountEquity, RiskPercentage, PricePoint};
    use rust_decimal_macros::dec;
    use proptest::prelude::*;

    fn create_valid_trade_proposal() -> TradeProposal {
        TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(), // 4% risk distance
            Some(PricePoint::new(dec!(54000)).unwrap()), // 2:1 reward/risk ratio
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(), // 2% risk - within limits
        ).unwrap()
    }

    #[test]
    fn test_max_trade_risk_rule_creation() {
        let rule = MaxTradeRiskRule::new();
        assert_eq!(rule.rule_name(), "MaxTradeRisk");
        assert!(!rule.description().is_empty());
    }

    #[test]
    fn test_max_trade_risk_rule_variants() {
        let standard_rule = MaxTradeRiskRule::new();
        let conservative_rule = MaxTradeRiskRule::conservative();
        let aggressive_rule = MaxTradeRiskRule::aggressive();
        
        // Verify different limits are applied
        assert!(conservative_rule.limits.max_individual_trade_risk < standard_rule.limits.max_individual_trade_risk);
        assert!(standard_rule.limits.max_individual_trade_risk < aggressive_rule.limits.max_individual_trade_risk);
    }

    #[test]
    fn test_valid_trade_assessment_approved() {
        let rule = MaxTradeRiskRule::new();
        let proposal = create_valid_trade_proposal();
        
        let result = rule.assess(&proposal);
        assert!(result.is_ok());
        
        let assessment = result.unwrap();
        assert_eq!(assessment.proposal_id, proposal.id);
        assert!(assessment.is_approved());
        assert_eq!(assessment.approval_status, ApprovalStatus::Approved);
        assert!(assessment.violations.is_empty());
        assert!(assessment.position_size.value() > Decimal::ZERO);
        assert!(assessment.risk_amount > Decimal::ZERO);
        assert_eq!(assessment.risk_percentage, dec!(0.02));
        assert!(assessment.reasoning.is_some());
        
        // Verify the reasoning contains approval confirmation
        let reasoning = assessment.reasoning.unwrap();
        assert!(reasoning.contains("approved"));
        assert!(reasoning.contains("within protocol limits"));
    }

    #[test]
    fn test_excessive_risk_assessment_rejected() {
        let rule = MaxTradeRiskRule::new();
        
        let high_risk_proposal = TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(),
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.08)).unwrap(), // 8% risk - exceeds 6% limit
        ).unwrap();
        
        let result = rule.assess(&high_risk_proposal);
        assert!(result.is_ok());
        
        let assessment = result.unwrap();
        assert!(!assessment.is_approved());
        assert!(!assessment.violations.is_empty());
        
        // Verify the violation is about max trade risk
        let violation = &assessment.violations[0];
        assert_eq!(violation.rule_name, "MaxTradeRisk");
        assert_eq!(violation.severity, ViolationSeverity::Critical);
        assert!(violation.description.contains("exceeds maximum allowed"));
        assert_eq!(violation.current_value, dec!(0.08));
        assert_eq!(violation.limit_value, dec!(0.06)); // Default max limit
        
        // Verify rejection reasoning
        let reasoning = assessment.reasoning.unwrap();
        assert!(reasoning.contains("rejected"));
        assert!(reasoning.contains("exceeds maximum"));
    }

    #[test]
    fn test_conservative_rule_stricter_limits() {
        let conservative_rule = MaxTradeRiskRule::conservative();
        
        // 3% risk should be rejected by conservative rule but allowed by standard
        let moderate_risk_proposal = TradeProposal::new(
            "ETHUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(3000)).unwrap(),
            PricePoint::new(dec!(2910)).unwrap(), // 3% risk distance
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.03)).unwrap(), // 3% risk
        ).unwrap();
        
        let conservative_assessment = conservative_rule.assess(&moderate_risk_proposal).unwrap();
        
        // Conservative rule should reject 3% risk (limit is 2%)
        assert!(!conservative_assessment.is_approved());
        assert!(!conservative_assessment.violations.is_empty());
        
        // Standard rule should approve the same trade
        let standard_rule = MaxTradeRiskRule::new();
        let standard_assessment = standard_rule.assess(&moderate_risk_proposal).unwrap();
        assert!(standard_assessment.is_approved());
    }

    #[test]
    fn test_position_size_calculation_accuracy() {
        let rule = MaxTradeRiskRule::new();
        let proposal = create_valid_trade_proposal();
        
        let assessment = rule.assess(&proposal).unwrap();
        
        // Verify Van Tharp position sizing formula
        // Position Size = (Account Equity × Risk %) ÷ (Entry - Stop)
        let expected_position_size = (dec!(10000) * dec!(0.02)) / dec!(2000); // $200 / $2000 = 0.1
        assert_eq!(assessment.position_size.value(), expected_position_size);
        
        // Verify risk amount calculation
        let expected_risk_amount = expected_position_size * dec!(2000); // 0.1 * $2000 = $200
        assert_eq!(assessment.risk_amount, expected_risk_amount);
    }

    // Property-based tests for mathematical accuracy (following Testudo Protocol)
    proptest! {
        #[test]
        fn prop_position_sizing_accuracy(
            account_equity in 1000.0..100000.0f64,
            risk_pct in 0.005..0.06f64, // Valid risk range
            entry_price in 10.0..100000.0f64,
            stop_distance in 1.0..1000.0f64,
        ) {
            let rule = MaxTradeRiskRule::new();
            
            let proposal = TradeProposal::new(
                "TESTUSDT".to_string(),
                TradeSide::Long,
                PricePoint::new(Decimal::from_f64(entry_price).unwrap()).unwrap(),
                PricePoint::new(Decimal::from_f64(entry_price - stop_distance).unwrap()).unwrap(),
                None,
                AccountEquity::new(Decimal::from_f64(account_equity).unwrap()).unwrap(),
                RiskPercentage::new(Decimal::from_f64(risk_pct).unwrap()).unwrap(),
            ).unwrap();
            
            let result = rule.assess(&proposal);
            prop_assert!(result.is_ok());
            
            let assessment = result.unwrap();
            
            // Property 1: Risk amount must equal calculated position size * risk distance
            let calculated_risk = assessment.position_size.value() * Decimal::from_f64(stop_distance).unwrap();
            prop_assert!((assessment.risk_amount - calculated_risk).abs() < Decimal::from_f64(0.01).unwrap());
            
            // Property 2: Risk percentage must be preserved
            prop_assert_eq!(assessment.risk_percentage, Decimal::from_f64(risk_pct).unwrap());
            
            // Property 3: Position size must be positive
            prop_assert!(assessment.position_size.value() > Decimal::ZERO);
            
            // Property 4: If risk is within limits, trade should be approved
            if risk_pct <= 0.06 {
                prop_assert!(assessment.is_approved());
            } else {
                prop_assert!(!assessment.is_approved());
            }
        }

        #[test]
        fn prop_risk_limit_enforcement(
            risk_pct in 0.07..0.20f64, // Above maximum limit
        ) {
            let rule = MaxTradeRiskRule::new();
            
            let proposal = TradeProposal::new(
                "TESTUSDT".to_string(),
                TradeSide::Long,
                PricePoint::new(dec!(50000)).unwrap(),
                PricePoint::new(dec!(48000)).unwrap(),
                None,
                AccountEquity::new(dec!(10000)).unwrap(),
                RiskPercentage::new(Decimal::from_f64(risk_pct).unwrap()).unwrap(),
            ).unwrap();
            
            let assessment = rule.assess(&proposal).unwrap();
            
            // Property: All trades with risk > 6% must be rejected
            prop_assert!(!assessment.is_approved());
            prop_assert!(!assessment.violations.is_empty());
            
            // Property: Violation must be about max trade risk
            prop_assert!(assessment.violations.iter().any(|v| v.rule_name == "MaxTradeRisk"));
        }
    }
}