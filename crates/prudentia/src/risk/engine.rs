//! Risk engine for comprehensive trade validation
//!
//! The RiskEngine is the central component that orchestrates risk validation
//! by applying multiple risk rules and generating comprehensive risk assessments.

use crate::types::{TradeProposal, RiskAssessment, ProtocolLimits, ApprovalStatus, ProtocolViolation, ViolationSeverity};
use crate::risk::rules::{RiskRule, RiskViolation};
use crate::risk::rules::{
    MaxIndividualTradeRiskRule, MinIndividualTradeRiskRule, MinRewardRiskRatioRule,
    StopLossDirectionRule, TakeProfitDirectionRule, ValidSymbolRule,
};
use disciplina::{PositionSizingCalculator, PositionSize};
use rust_decimal::Decimal;
use std::sync::Arc;
use tracing::{info, warn, error};

/// Comprehensive risk validation engine
///
/// The RiskEngine applies multiple risk rules to trade proposals and generates
/// detailed risk assessments. It integrates with the Van Tharp position sizing
/// calculator from the disciplina crate.
#[derive(Debug)]
pub struct RiskEngine {
    /// Van Tharp position sizing calculator
    position_calculator: Arc<PositionSizingCalculator>,
    /// Protocol limits configuration
    protocol_limits: ProtocolLimits,
    /// List of risk rules to apply (ordered by priority)
    risk_rules: Vec<Box<dyn RiskRule>>,
}

impl RiskEngine {
    /// Create a new risk engine with default configuration
    pub fn new() -> Self {
        let protocol_limits = ProtocolLimits::default();
        Self::with_limits(protocol_limits)
    }
    
    /// Create a new risk engine with custom protocol limits
    pub fn with_limits(protocol_limits: ProtocolLimits) -> Self {
        let position_calculator = Arc::new(PositionSizingCalculator::new());
        
        // Create standard risk rules with the given limits
        let mut risk_rules: Vec<Box<dyn RiskRule>> = vec![
            Box::new(ValidSymbolRule),
            Box::new(StopLossDirectionRule),
            Box::new(TakeProfitDirectionRule),
            Box::new(MaxIndividualTradeRiskRule::new(protocol_limits.clone())),
            Box::new(MinRewardRiskRatioRule::new(protocol_limits.clone())),
            Box::new(MinIndividualTradeRiskRule::new(protocol_limits.clone())),
        ];
        
        // Sort rules by priority (lower number = higher priority)
        risk_rules.sort_by_key(|rule| rule.priority());
        
        Self {
            position_calculator,
            protocol_limits,
            risk_rules,
        }
    }
    
    /// Create a conservative risk engine for new traders
    pub fn conservative() -> Self {
        Self::with_limits(ProtocolLimits::conservative_limits())
    }
    
    /// Create an aggressive risk engine for experienced traders
    pub fn aggressive() -> Self {
        Self::with_limits(ProtocolLimits::aggressive_limits())
    }
    
    /// Add a custom risk rule to the engine
    pub fn add_rule(&mut self, rule: Box<dyn RiskRule>) {
        self.risk_rules.push(rule);
        // Re-sort by priority after adding new rule
        self.risk_rules.sort_by_key(|rule| rule.priority());
    }
    
    /// Perform comprehensive risk assessment on a trade proposal
    pub fn assess_trade(&self, proposal: &TradeProposal) -> RiskAssessment {
        info!("Starting risk assessment for trade proposal: {}", proposal.id);
        
        // Step 1: Calculate position size using Van Tharp methodology
        let position_size = match self.calculate_position_size(proposal) {
            Ok(size) => size,
            Err(err) => {
                error!("Failed to calculate position size: {}", err);
                // Create a failed assessment with blocking status
                let mut assessment = RiskAssessment::new(
                    proposal.id,
                    PositionSize::new(Decimal::ZERO).unwrap_or_else(|_| unreachable!()),
                    Decimal::ZERO,
                    proposal.risk_percentage.value(),
                    proposal.risk_reward_ratio(),
                    Decimal::ZERO,
                );
                
                let violation = ProtocolViolation::new(
                    "PositionSizeCalculation".to_string(),
                    ViolationSeverity::Blocking,
                    format!("Failed to calculate position size: {}", err),
                    Decimal::ZERO,
                    Decimal::ONE,
                    "Check trade proposal parameters and ensure they are valid".to_string(),
                );
                
                assessment.add_violation(violation);
                return assessment;
            }
        };
        
        // Step 2: Calculate risk metrics
        let risk_amount = self.calculate_risk_amount(proposal, &position_size);
        let portfolio_impact = self.calculate_portfolio_impact(proposal, &risk_amount);
        
        // Step 3: Create initial assessment
        let mut assessment = RiskAssessment::new(
            proposal.id,
            position_size,
            risk_amount,
            proposal.risk_percentage.value(),
            proposal.risk_reward_ratio(),
            portfolio_impact,
        );
        
        // Step 4: Apply all risk rules
        let mut total_violations = 0;
        let mut blocking_violations = 0;
        
        for rule in &self.risk_rules {
            match rule.validate(proposal) {
                Ok(()) => {
                    info!("Rule '{}' passed for proposal {}", rule.rule_name(), proposal.id);
                }
                Err(violation) => {
                    warn!(
                        "Rule '{}' violated for proposal {}: {}",
                        rule.rule_name(),
                        proposal.id,
                        violation.description
                    );
                    
                    total_violations += 1;
                    if violation.severity == ViolationSeverity::Blocking {
                        blocking_violations += 1;
                    }
                    
                    assessment.add_violation(violation.to_protocol_violation());
                }
            }
        }
        
        // Step 5: Add assessment reasoning
        let reasoning = self.generate_assessment_reasoning(&assessment, total_violations, blocking_violations);
        let final_assessment = assessment.with_reasoning(reasoning);
        
        info!(
            "Risk assessment completed for proposal {}: {} (violations: {})",
            proposal.id,
            final_assessment.approval_status,
            total_violations
        );
        
        final_assessment
    }
    
    /// Calculate position size using Van Tharp methodology
    fn calculate_position_size(&self, proposal: &TradeProposal) -> disciplina::Result<PositionSize> {
        self.position_calculator.calculate_position_size(
            proposal.account_equity,
            proposal.risk_percentage,
            proposal.entry_price,
            proposal.stop_loss,
        )
    }
    
    /// Calculate dollar amount at risk
    fn calculate_risk_amount(&self, proposal: &TradeProposal, position_size: &PositionSize) -> Decimal {
        let risk_per_share = proposal.risk_distance();
        position_size.value() * risk_per_share
    }
    
    /// Calculate impact on total portfolio risk
    /// For now, this is a simplified calculation - in a real system,
    /// this would consider existing open positions
    fn calculate_portfolio_impact(&self, proposal: &TradeProposal, risk_amount: &Decimal) -> Decimal {
        // Simple calculation: risk amount as percentage of account equity
        risk_amount / proposal.account_equity.value()
    }
    
    /// Generate human-readable reasoning for the assessment
    fn generate_assessment_reasoning(
        &self,
        assessment: &RiskAssessment,
        total_violations: usize,
        blocking_violations: usize,
    ) -> String {
        match assessment.approval_status {
            ApprovalStatus::Approved => {
                format!(
                    "Trade approved: Position size {} calculated using Van Tharp methodology. \
                     Risk amount ${:.2} ({:.1}% of account). All {} risk rules passed.",
                    assessment.position_size.value(),
                    assessment.risk_amount,
                    assessment.risk_percentage * Decimal::from(100),
                    self.risk_rules.len()
                )
            }
            ApprovalStatus::ApprovedWithWarnings => {
                format!(
                    "Trade approved with warnings: Position size {} calculated. \
                     Risk amount ${:.2} ({:.1}% of account). {} warnings found - review recommended.",
                    assessment.position_size.value(),
                    assessment.risk_amount,
                    assessment.risk_percentage * Decimal::from(100),
                    total_violations
                )
            }
            ApprovalStatus::RequiresReduction => {
                format!(
                    "Trade requires modification: Original position size {} would risk ${:.2}. \
                     {} violations found requiring position reduction or parameter adjustment.",
                    assessment.position_size.value(),
                    assessment.risk_amount,
                    total_violations
                )
            }
            ApprovalStatus::Rejected => {
                format!(
                    "Trade rejected: {} critical violations found. \
                     Risk management protocol prevents execution without significant modifications.",
                    total_violations
                )
            }
            ApprovalStatus::Blocked => {
                format!(
                    "Trade blocked: {} blocking violations found (including {} blocking issues). \
                     Trade cannot be executed under current conditions.",
                    total_violations,
                    blocking_violations
                )
            }
        }
    }
    
    /// Get the current protocol limits
    pub fn protocol_limits(&self) -> &ProtocolLimits {
        &self.protocol_limits
    }
    
    /// Get the number of active risk rules
    pub fn rule_count(&self) -> usize {
        self.risk_rules.len()
    }
    
    /// Get information about all active risk rules
    pub fn rule_info(&self) -> Vec<(String, u8, String)> {
        self.risk_rules
            .iter()
            .map(|rule| {
                (
                    rule.rule_name().to_string(),
                    rule.priority(),
                    rule.description().to_string(),
                )
            })
            .collect()
    }
}

impl Default for RiskEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TradeSide};
    use disciplina::{AccountEquity, RiskPercentage, PricePoint};
    use rust_decimal_macros::dec;
    
    fn create_valid_proposal() -> TradeProposal {
        TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(), // 2000 risk distance
            Some(PricePoint::new(dec!(54000)).unwrap()), // 4000 reward distance (2:1 ratio)
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(), // 2% risk
        ).unwrap()
    }
    
    #[test]
    fn test_risk_engine_creation() {
        let engine = RiskEngine::new();
        assert!(engine.rule_count() > 0);
        assert_eq!(engine.protocol_limits().max_individual_trade_risk, dec!(0.06));
        
        let conservative = RiskEngine::conservative();
        assert_eq!(conservative.protocol_limits().max_individual_trade_risk, dec!(0.02));
        
        let aggressive = RiskEngine::aggressive();
        assert_eq!(aggressive.protocol_limits().max_individual_trade_risk, dec!(0.10));
    }
    
    #[test]
    fn test_valid_trade_assessment() {
        let engine = RiskEngine::new();
        let proposal = create_valid_proposal();
        
        let assessment = engine.assess_trade(&proposal);
        
        assert_eq!(assessment.proposal_id, proposal.id);
        assert!(assessment.is_approved());
        assert!(assessment.position_size.value() > Decimal::ZERO);
        assert!(assessment.risk_amount > Decimal::ZERO);
        assert_eq!(assessment.risk_percentage, dec!(0.02));
        assert!(assessment.reasoning.is_some());
    }
    
    #[test]
    fn test_excessive_risk_rejection() {
        let engine = RiskEngine::new();
        
        let high_risk_proposal = TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(),
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.08)).unwrap(), // 8% risk - exceeds 6% limit
        ).unwrap();
        
        let assessment = engine.assess_trade(&high_risk_proposal);
        
        assert!(!assessment.is_approved());
        assert!(assessment.is_blocked() || !assessment.is_approved());
        assert!(!assessment.violations.is_empty());
        
        // Should have MaxIndividualTradeRisk violation
        let has_max_risk_violation = assessment
            .violations
            .iter()
            .any(|v| v.rule_name == "MaxIndividualTradeRisk");
        assert!(has_max_risk_violation);
    }
    
    #[test]
    fn test_invalid_stop_loss_blocking() {
        let engine = RiskEngine::new();
        
        let invalid_proposal = TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(52000)).unwrap(), // Stop above entry (invalid for long)
            None,
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(),
        ).unwrap();
        
        let assessment = engine.assess_trade(&invalid_proposal);
        
        assert!(assessment.is_blocked());
        assert_eq!(assessment.approval_status, ApprovalStatus::Blocked);
        
        // Should have StopLossDirection violation
        let has_stop_violation = assessment
            .violations
            .iter()
            .any(|v| v.rule_name == "StopLossDirection" && v.severity == ViolationSeverity::Blocking);
        assert!(has_stop_violation);
    }
    
    #[test]
    fn test_poor_reward_risk_ratio_warning() {
        let engine = RiskEngine::new();
        
        let poor_ratio_proposal = TradeProposal::new(
            "BTCUSDT".to_string(),
            TradeSide::Long,
            PricePoint::new(dec!(50000)).unwrap(),
            PricePoint::new(dec!(48000)).unwrap(), // 2000 risk
            Some(PricePoint::new(dec!(51000)).unwrap()), // 1000 reward = 0.5:1 ratio (below 2:1 requirement)
            AccountEquity::new(dec!(10000)).unwrap(),
            RiskPercentage::new(dec!(0.02)).unwrap(),
        ).unwrap();
        
        let assessment = engine.assess_trade(&poor_ratio_proposal);
        
        assert!(assessment.requires_modification() || !assessment.is_approved());
        
        // Should have MinRewardRiskRatio violation
        let has_ratio_violation = assessment
            .violations
            .iter()
            .any(|v| v.rule_name == "MinRewardRiskRatio");
        assert!(has_ratio_violation);
    }
    
    #[test]
    fn test_custom_rule_addition() {
        let mut engine = RiskEngine::new();
        let initial_count = engine.rule_count();
        
        // Add a custom rule (we'll reuse an existing rule for testing)
        let custom_rule = Box::new(ValidSymbolRule);
        engine.add_rule(custom_rule);
        
        assert_eq!(engine.rule_count(), initial_count + 1);
    }
    
    #[test]
    fn test_rule_priority_ordering() {
        let engine = RiskEngine::new();
        let rule_info = engine.rule_info();
        
        // Rules should be ordered by priority (lower numbers first)
        for window in rule_info.windows(2) {
            let (_, priority1, _) = &window[0];
            let (_, priority2, _) = &window[1];
            assert!(priority1 <= priority2);
        }
    }
    
    #[test]
    fn test_assessment_reasoning() {
        let engine = RiskEngine::new();
        let proposal = create_valid_proposal();
        
        let assessment = engine.assess_trade(&proposal);
        
        assert!(assessment.reasoning.is_some());
        let reasoning = assessment.reasoning.unwrap();
        assert!(reasoning.contains("Van Tharp"));
        assert!(reasoning.contains("risk rules"));
    }
}