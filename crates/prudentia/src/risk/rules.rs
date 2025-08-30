//! Risk rules and validation logic
//!
//! This module defines the trait and implementations for various risk rules
//! that are applied to trade proposals during validation.

use crate::types::{TradeProposal, ProtocolLimits, ProtocolViolation, ViolationSeverity};
use rust_decimal::Decimal;
use std::fmt;

/// Core trait for risk validation rules
///
/// Each rule represents a specific risk constraint that must be checked
/// before allowing a trade to be executed. Rules can have different
/// priorities and severities.
pub trait RiskRule: Send + Sync + fmt::Debug {
    /// Validate a trade proposal against this rule
    fn validate(&self, proposal: &TradeProposal) -> Result<(), RiskViolation>;
    
    /// Get the name of this rule for logging and error reporting
    fn rule_name(&self) -> &str;
    
    /// Get the priority of this rule (lower numbers = higher priority)
    /// Rules with higher priority are checked first
    fn priority(&self) -> u8;
    
    /// Get a human-readable description of what this rule checks
    fn description(&self) -> &str;
}

/// Violation of a risk rule
#[derive(Debug, Clone, PartialEq)]
pub struct RiskViolation {
    /// Name of the rule that was violated
    pub rule_name: String,
    /// Severity of the violation
    pub severity: ViolationSeverity,
    /// Detailed description of the violation
    pub description: String,
    /// Current value that caused the violation
    pub current_value: Decimal,
    /// Limit value that was exceeded
    pub limit_value: Decimal,
    /// Suggested action to resolve the violation
    pub suggested_action: String,
}

impl RiskViolation {
    /// Create a new risk violation
    pub fn new(
        rule_name: String,
        severity: ViolationSeverity,
        description: String,
        current_value: Decimal,
        limit_value: Decimal,
        suggested_action: String,
    ) -> Self {
        Self {
            rule_name,
            severity,
            description,
            current_value,
            limit_value,
            suggested_action,
        }
    }
    
    /// Convert this violation to a ProtocolViolation
    pub fn to_protocol_violation(self) -> ProtocolViolation {
        ProtocolViolation::new(
            self.rule_name,
            self.severity,
            self.description,
            self.current_value,
            self.limit_value,
            self.suggested_action,
        )
    }
}

/// Rule that validates individual trade risk percentage
#[derive(Debug, Clone)]
pub struct MaxIndividualTradeRiskRule {
    limits: ProtocolLimits,
}

impl MaxIndividualTradeRiskRule {
    pub fn new(limits: ProtocolLimits) -> Self {
        Self { limits }
    }
}

impl RiskRule for MaxIndividualTradeRiskRule {
    fn validate(&self, proposal: &TradeProposal) -> Result<(), RiskViolation> {
        let risk_percentage = proposal.risk_percentage.value();
        
        if risk_percentage > self.limits.max_individual_trade_risk {
            return Err(RiskViolation::new(
                self.rule_name().to_string(),
                ViolationSeverity::Critical,
                format!(
                    "Individual trade risk {}% exceeds maximum allowed {}%",
                    risk_percentage * Decimal::from(100),
                    self.limits.max_individual_trade_risk * Decimal::from(100)
                ),
                risk_percentage,
                self.limits.max_individual_trade_risk,
                format!(
                    "Reduce position risk to maximum {}% of account equity",
                    self.limits.max_individual_trade_risk * Decimal::from(100)
                ),
            ));
        }
        
        Ok(())
    }
    
    fn rule_name(&self) -> &str {
        "MaxIndividualTradeRisk"
    }
    
    fn priority(&self) -> u8 {
        1 // High priority - fundamental risk constraint
    }
    
    fn description(&self) -> &str {
        "Ensures individual trade risk does not exceed maximum protocol limit"
    }
}

/// Rule that validates minimum individual trade risk percentage
#[derive(Debug, Clone)]
pub struct MinIndividualTradeRiskRule {
    limits: ProtocolLimits,
}

impl MinIndividualTradeRiskRule {
    pub fn new(limits: ProtocolLimits) -> Self {
        Self { limits }
    }
}

impl RiskRule for MinIndividualTradeRiskRule {
    fn validate(&self, proposal: &TradeProposal) -> Result<(), RiskViolation> {
        let risk_percentage = proposal.risk_percentage.value();
        
        if risk_percentage < self.limits.min_individual_trade_risk {
            return Err(RiskViolation::new(
                self.rule_name().to_string(),
                ViolationSeverity::Warning,
                format!(
                    "Individual trade risk {}% below minimum recommended {}%",
                    risk_percentage * Decimal::from(100),
                    self.limits.min_individual_trade_risk * Decimal::from(100)
                ),
                risk_percentage,
                self.limits.min_individual_trade_risk,
                format!(
                    "Consider increasing position risk to at least {}% for meaningful trades",
                    self.limits.min_individual_trade_risk * Decimal::from(100)
                ),
            ));
        }
        
        Ok(())
    }
    
    fn rule_name(&self) -> &str {
        "MinIndividualTradeRisk"
    }
    
    fn priority(&self) -> u8 {
        5 // Lower priority - this is a warning, not a blocker
    }
    
    fn description(&self) -> &str {
        "Ensures individual trade risk meets minimum meaningful threshold"
    }
}

/// Rule that validates reward-to-risk ratio
#[derive(Debug, Clone)]
pub struct MinRewardRiskRatioRule {
    limits: ProtocolLimits,
}

impl MinRewardRiskRatioRule {
    pub fn new(limits: ProtocolLimits) -> Self {
        Self { limits }
    }
}

impl RiskRule for MinRewardRiskRatioRule {
    fn validate(&self, proposal: &TradeProposal) -> Result<(), RiskViolation> {
        // Only validate if take profit is set
        if let Some(ratio) = proposal.risk_reward_ratio() {
            if ratio < self.limits.min_reward_risk_ratio {
                return Err(RiskViolation::new(
                    self.rule_name().to_string(),
                    ViolationSeverity::High,
                    format!(
                        "Reward-to-risk ratio {:.2} below minimum required {:.2}",
                        ratio,
                        self.limits.min_reward_risk_ratio
                    ),
                    ratio,
                    self.limits.min_reward_risk_ratio,
                    format!(
                        "Adjust take profit to achieve at least {:.1}:1 reward-to-risk ratio",
                        self.limits.min_reward_risk_ratio
                    ),
                ));
            }
        }
        // If no take profit is set, we don't validate (trade can run without target)
        
        Ok(())
    }
    
    fn rule_name(&self) -> &str {
        "MinRewardRiskRatio"
    }
    
    fn priority(&self) -> u8 {
        3 // Medium priority - important for long-term profitability
    }
    
    fn description(&self) -> &str {
        "Ensures trades have favorable reward-to-risk ratio when take profit is set"
    }
}

/// Rule that validates stop loss direction is correct for trade side
#[derive(Debug, Clone)]
pub struct StopLossDirectionRule;

impl RiskRule for StopLossDirectionRule {
    fn validate(&self, proposal: &TradeProposal) -> Result<(), RiskViolation> {
        let entry = proposal.entry_price.value();
        let stop = proposal.stop_loss.value();
        
        let violation = match proposal.side {
            crate::types::TradeSide::Long => {
                if stop >= entry {
                    Some("For long positions, stop loss must be below entry price")
                } else {
                    None
                }
            }
            crate::types::TradeSide::Short => {
                if stop <= entry {
                    Some("For short positions, stop loss must be above entry price")
                } else {
                    None
                }
            }
        };
        
        if let Some(error_msg) = violation {
            return Err(RiskViolation::new(
                self.rule_name().to_string(),
                ViolationSeverity::Blocking,
                error_msg.to_string(),
                stop,
                entry,
                format!(
                    "Adjust stop loss to be on correct side of entry price for {} trade",
                    proposal.side
                ),
            ));
        }
        
        Ok(())
    }
    
    fn rule_name(&self) -> &str {
        "StopLossDirection"
    }
    
    fn priority(&self) -> u8 {
        0 // Highest priority - fundamental trade logic error
    }
    
    fn description(&self) -> &str {
        "Validates that stop loss is on the correct side of entry price"
    }
}

/// Rule that validates take profit direction is correct for trade side (if set)
#[derive(Debug, Clone)]
pub struct TakeProfitDirectionRule;

impl RiskRule for TakeProfitDirectionRule {
    fn validate(&self, proposal: &TradeProposal) -> Result<(), RiskViolation> {
        if let Some(take_profit) = proposal.take_profit {
            let entry = proposal.entry_price.value();
            let tp = take_profit.value();
            
            let violation = match proposal.side {
                crate::types::TradeSide::Long => {
                    if tp <= entry {
                        Some("For long positions, take profit must be above entry price")
                    } else {
                        None
                    }
                }
                crate::types::TradeSide::Short => {
                    if tp >= entry {
                        Some("For short positions, take profit must be below entry price")
                    } else {
                        None
                    }
                }
            };
            
            if let Some(error_msg) = violation {
                return Err(RiskViolation::new(
                    self.rule_name().to_string(),
                    ViolationSeverity::Blocking,
                    error_msg.to_string(),
                    tp,
                    entry,
                    format!(
                        "Adjust take profit to be on correct side of entry price for {} trade",
                        proposal.side
                    ),
                ));
            }
        }
        
        Ok(())
    }
    
    fn rule_name(&self) -> &str {
        "TakeProfitDirection"
    }
    
    fn priority(&self) -> u8 {
        0 // Highest priority - fundamental trade logic error
    }
    
    fn description(&self) -> &str {
        "Validates that take profit is on the correct side of entry price when set"
    }
}

/// Rule that validates the symbol is not empty or invalid
#[derive(Debug, Clone)]
pub struct ValidSymbolRule;

impl RiskRule for ValidSymbolRule {
    fn validate(&self, proposal: &TradeProposal) -> Result<(), RiskViolation> {
        if proposal.symbol.is_empty() {
            return Err(RiskViolation::new(
                self.rule_name().to_string(),
                ViolationSeverity::Blocking,
                "Trading symbol cannot be empty".to_string(),
                Decimal::ZERO,
                Decimal::ONE,
                "Provide a valid trading symbol (e.g., BTCUSDT, ETHUSDT)".to_string(),
            ));
        }
        
        // Additional symbol validation could be added here
        // (e.g., format checking, whitelist validation, etc.)
        
        Ok(())
    }
    
    fn rule_name(&self) -> &str {
        "ValidSymbol"
    }
    
    fn priority(&self) -> u8 {
        0 // Highest priority - basic validation
    }
    
    fn description(&self) -> &str {
        "Validates that trading symbol is provided and valid"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TradeSide};
    use disciplina::{AccountEquity, RiskPercentage, PricePoint};
    use rust_decimal_macros::dec;
    
    fn create_test_proposal() -> TradeProposal {
        TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(), // 4% risk distance
            Some(PricePoint::new(dec!(54000)).unwrap()), // 8% reward (2:1 ratio)
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(), // 2% risk
        ).unwrap()
    }
    
    #[test]
    fn test_max_individual_trade_risk_rule() {
        let limits = ProtocolLimits::default();
        let rule = MaxIndividualTradeRiskRule::new(limits);
        let proposal = create_test_proposal();
        
        // Should pass with 2% risk (under 6% limit)
        assert!(rule.validate(&proposal).is_ok());
        
        // Test with excessive risk
        let high_risk_proposal = TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(),
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.08)).unwrap(), // 8% risk - exceeds 6% limit
        ).unwrap();
        
        let result = rule.validate(&high_risk_proposal);
        assert!(result.is_err());
        
        let violation = result.unwrap_err();
        assert_eq!(violation.rule_name, "MaxIndividualTradeRisk");
        assert_eq!(violation.severity, ViolationSeverity::Critical);
    }
    
    #[test]
    fn test_min_reward_risk_ratio_rule() {
        let limits = ProtocolLimits::default();
        let rule = MinRewardRiskRatioRule::new(limits);
        let proposal = create_test_proposal();
        
        // Should pass with 2:1 ratio (meets 2:1 minimum)
        assert!(rule.validate(&proposal).is_ok());
        
        // Test with poor risk/reward ratio
        let poor_ratio_proposal = TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(), // 2000 risk
            Some(PricePoint::new(dec!(51000)).unwrap()), // 1000 reward = 0.5:1 ratio
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(),
        ).unwrap();
        
        let result = rule.validate(&poor_ratio_proposal);
        assert!(result.is_err());
        
        let violation = result.unwrap_err();
        assert_eq!(violation.rule_name, "MinRewardRiskRatio");
        assert_eq!(violation.severity, ViolationSeverity::High);
    }
    
    #[test]
    fn test_stop_loss_direction_rule() {
        let rule = StopLossDirectionRule;
        let proposal = create_test_proposal();
        
        // Should pass with correct stop loss direction for long
        assert!(rule.validate(&proposal).is_ok());
        
        // Test with incorrect stop loss direction for long
        let bad_long_proposal = TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(52000)).unwrap(), // Stop above entry (wrong for long)
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(),
        ).unwrap();
        
        let result = rule.validate(&bad_long_proposal);
        assert!(result.is_err());
        
        let violation = result.unwrap_err();
        assert_eq!(violation.rule_name, "StopLossDirection");
        assert_eq!(violation.severity, ViolationSeverity::Blocking);
    }
    
    #[test]
    fn test_valid_symbol_rule() {
        let rule = ValidSymbolRule;
        let proposal = create_test_proposal();
        
        // Should pass with valid symbol
        assert!(rule.validate(&proposal).is_ok());
        
        // Test with empty symbol
        let empty_symbol_proposal = TradeProposal::new(
            "".to_string(), // Empty symbol
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(),
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(),
        ).unwrap();
        
        let result = rule.validate(&empty_symbol_proposal);
        assert!(result.is_err());
        
        let violation = result.unwrap_err();
        assert_eq!(violation.rule_name, "ValidSymbol");
        assert_eq!(violation.severity, ViolationSeverity::Blocking);
    }
    
    #[test]
    fn test_rule_priorities() {
        let limits = ProtocolLimits::default();
        
        let symbol_rule = ValidSymbolRule;
        let stop_rule = StopLossDirectionRule;
        let min_risk_rule = MinIndividualTradeRiskRule::new(limits.clone());
        let max_risk_rule = MaxIndividualTradeRiskRule::new(limits);
        
        // Verify priority ordering (lower number = higher priority)
        assert_eq!(symbol_rule.priority(), 0);
        assert_eq!(stop_rule.priority(), 0);
        assert!(max_risk_rule.priority() < min_risk_rule.priority());
    }
}